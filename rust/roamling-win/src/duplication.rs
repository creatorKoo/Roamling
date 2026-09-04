// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Reading the screen without stalling on it.
//!
//! `BitBlt` from a screen DC measured 256-1117 ms per readback and did not
//! scale with area, because on a composited desktop it forces a GPU-to-CPU
//! sync no matter how few pixels are asked for. `docs/windows.md` section 5
//! carries the numbers. Desktop Duplication is the path that does not: the
//! compositor hands over the frame it already has.
//!
//! Two things fall out of using it that `BitBlt` did not give:
//!
//! - **Change detection is free.** `AcquireNextFrame` times out when nothing
//!   has been drawn since the last call, and a screen that has not changed
//!   still has the luminance it had. Most calls cost nothing at all.
//! - **A locked screen says so.** The duplication is lost rather than quietly
//!   returning black, which is the failure `capture::screen_is_readable` had to
//!   guard against by hand.

use windows::core::Interface;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
    D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAPPED_SUBRESOURCE,
    D3D11_MAP_READ, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIFactory1, IDXGIOutput1, IDXGIOutputDuplication, IDXGIResource,
    DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_WAIT_TIMEOUT, DXGI_OUTDUPL_FRAME_INFO,
};

pub struct Duplicated {
    /// Row-major BGRA, top row first. Borrowed from the mapped staging texture,
    /// so it only lives until `release`.
    pub width: usize,
    pub height: usize,
    pub stride: usize,
}

/// One display's duplication, kept alive across captures.
///
/// Creating this costs far more than using it, and `AcquireNextFrame` only
/// answers for a duplication that has been held open, so it is built once and
/// rebuilt only when Windows takes it away.
pub struct Duplication {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    output: IDXGIOutput1,
    duplication: IDXGIOutputDuplication,
    staging: Option<(ID3D11Texture2D, u32, u32)>,
    /// Set while a frame is held; `AcquireNextFrame` will not give another one
    /// until it is released.
    holding: bool,
}

impl Duplication {
    /// `device_name` is what `EnumDisplayMonitors` reported, like `\\.\DISPLAY1`.
    /// Matching on it is what picks the right adapter on a machine with two GPUs.
    pub fn open(device_name: &str) -> Option<Self> {
        unsafe {
            let factory: IDXGIFactory1 = CreateDXGIFactory1().ok()?;
            let mut adapter_index = 0;
            while let Ok(adapter) = factory.EnumAdapters1(adapter_index) {
                adapter_index += 1;
                let mut output_index = 0;
                while let Ok(output) = adapter.EnumOutputs(output_index) {
                    output_index += 1;
                    let Ok(description) = output.GetDesc() else {
                        continue;
                    };
                    let name = String::from_utf16_lossy(&description.DeviceName)
                        .trim_end_matches('\0')
                        .to_string();
                    if name != device_name {
                        continue;
                    }

                    // The device has to be on the adapter that drives this
                    // output, or `DuplicateOutput` refuses.
                    let mut device: Option<ID3D11Device> = None;
                    let mut context: Option<ID3D11DeviceContext> = None;
                    for driver in [D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP] {
                        let made = D3D11CreateDevice(
                            &adapter,
                            // A device made against an explicit adapter must
                            // say UNKNOWN, which is why the loop passes the
                            // driver type only as a fallback path.
                            windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_UNKNOWN,
                            None,
                            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                            None,
                            D3D11_SDK_VERSION,
                            Some(&mut device),
                            None,
                            Some(&mut context),
                        );
                        let _ = driver;
                        if made.is_ok() {
                            break;
                        }
                    }
                    let Some(device) = device else {
                        return None;
                    };
                    let Some(context) = context else {
                        return None;
                    };
                    let output1: IDXGIOutput1 = match output.cast() {
                        Ok(value) => value,
                        Err(_) => return None,
                    };
                    let duplication = match output1.DuplicateOutput(&device) {
                        Ok(value) => value,
                        Err(_) => return None,
                    };
                    return Some(Self {
                        device,
                        context,
                        output: output1,
                        duplication,
                        staging: None,
                        holding: false,
                    });
                }
            }
            None
        }
    }

    /// Windows takes the duplication away on a mode change, a lock, a
    /// full-screen transition, or a GPU reset. Reopening is the documented
    /// response, and it is why `open` keeps the output around.
    fn reopen(&mut self) -> bool {
        unsafe {
            match self.output.DuplicateOutput(&self.device) {
                Ok(fresh) => {
                    self.duplication = fresh;
                    self.holding = false;
                    true
                }
                Err(_) => false,
            }
        }
    }

    fn staging_for(&mut self, width: u32, height: u32) -> Option<ID3D11Texture2D> {
        if let Some((texture, w, h)) = &self.staging {
            if *w == width && *h == height {
                return Some(texture.clone());
            }
        }
        let description = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
        };
        let mut texture: Option<ID3D11Texture2D> = None;
        unsafe {
            self.device
                .CreateTexture2D(&description, None, Some(&mut texture))
                .ok()?;
        }
        let texture = texture?;
        self.staging = Some((texture.clone(), width, height));
        Some(texture)
    }

    /// Take the next frame, if the screen has drawn one, and hand its pixels to
    /// `read`. `Ok(false)` means nothing changed, which is not a failure --
    /// the caller's previous answer is still the right one.
    pub fn next_frame<R>(&mut self, read: R) -> Result<bool, ()>
    where
        R: FnOnce(&[u8], Duplicated),
    {
        unsafe {
            if self.holding {
                let _ = self.duplication.ReleaseFrame();
                self.holding = false;
            }

            let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut resource: Option<IDXGIResource> = None;
            // Zero timeout: this runs on the message loop and must never wait
            // for the compositor to draw something.
            match self.duplication.AcquireNextFrame(0, &mut info, &mut resource) {
                Ok(()) => {}
                Err(error) if error.code() == DXGI_ERROR_WAIT_TIMEOUT => return Ok(false),
                Err(error) if error.code() == DXGI_ERROR_ACCESS_LOST => {
                    // Locked screen, mode change, a full-screen app taking
                    // over. Rebuild and let the next tick try again.
                    return if self.reopen() { Ok(false) } else { Err(()) };
                }
                Err(_) => return Err(()),
            }
            self.holding = true;

            // A frame can arrive carrying nothing but a pointer move, and the
            // very first one after `DuplicateOutput` carries nothing at all --
            // the surface is whatever was in that memory, which is zeroes.
            // `LastPresentTime` is how the API says so: zero means the desktop
            // image was not updated, so there is nothing here to read.
            //
            // Reading it anyway produced an all-black field, and an all-black
            // field has no gradient and no spread -- which the seat scorer
            // reads as a *perfectly empty screen* and the pet takes as
            // permission to sit anywhere, including on the user's text. Every
            // capture session began with one.
            if info.LastPresentTime == 0 {
                return Ok(false);
            }

            let Some(resource) = resource else {
                return Err(());
            };
            let Ok(frame) = resource.cast::<ID3D11Texture2D>() else {
                return Err(());
            };
            let mut description = D3D11_TEXTURE2D_DESC::default();
            frame.GetDesc(&mut description);

            let Some(staging) = self.staging_for(description.Width, description.Height) else {
                return Err(());
            };
            self.context.CopyResource(&staging, &frame);

            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            if self
                .context
                .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                .is_err()
            {
                return Err(());
            }
            let stride = mapped.RowPitch as usize;
            let height = description.Height as usize;
            let pixels = std::slice::from_raw_parts(mapped.pData as *const u8, stride * height);
            read(
                pixels,
                Duplicated {
                    width: description.Width as usize,
                    height,
                    stride,
                },
            );
            self.context.Unmap(&staging, 0);
            Ok(true)
        }
    }
}

impl Drop for Duplication {
    fn drop(&mut self) {
        if self.holding {
            unsafe {
                let _ = self.duplication.ReleaseFrame();
            }
        }
    }
}
