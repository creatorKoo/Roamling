// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

// Generated from the pipeline as it stood before W2 replaced CoreGraphics
// with a byte blitter. Every entry is an FNV-1a hash of one frame as RGBA8
// premultiplied, top row first -- the exact bytes that used to reach the
// screen. A diff here is a rendering regression, which in a project with
// these art invariants is a defect rather than a tolerance.

enum PreW2FrameHashes {
    struct Asset {
        let name: String
        let atlas: String
        let extensionAtlas: String?
        /// index -> hash, in frame order.
        let frames: [String]
    }

    static let assets: [Asset] = [
        Asset(
            name: "built-in mochi",
            atlas: "099042fa23b1bd92",
            extensionAtlas: "b91016c252ce3218",
            frames: [
                "79535897cbad907a", "f807e227ded1c42a", "0e0c81eb75db0747", "8472b156b5882f3d",
                "8e492dfae3aeda51", "044653465838420a", "04dd55125dfae325", "04dd55125dfae325",
                "430ae8cdecf662da", "3f4f2fcb3302d7a3", "0ac92c30c96f6ba9", "dd24e2622353c41b",
                "e4c6ae6a3e17cb40", "d145c81daab2854b", "68e4637436c4ad9d", "609cc0e013aff30e",
                "fe6e6f5225bc3012", "14ce33fc9bc45893", "a32a3bcf6978a7bd", "6fe64b6ea85fc783",
                "85865ccf512a0aec", "c9e8529cb555a9c3", "589ae570baa04ec5", "d65efeaab4e85a6e",
                "f98a3682f13886e9", "f85266cb8682098f", "7666c2ff22e7c7ed", "f98a3682f13886e9",
                "04dd55125dfae325", "04dd55125dfae325", "04dd55125dfae325", "04dd55125dfae325",
                "08f68a5133d668b5", "f40eac7c22a04a7a", "11eb5804d3ee28d6", "d3e32ea10d819a5b",
                "9b700af78d63f1e8", "04dd55125dfae325", "04dd55125dfae325", "04dd55125dfae325",
                "c642da2cc34e2201", "962e25311ef7dc02", "541500c9ad85a1ad", "3297964d5c250f95",
                "44ce495721a35bba", "67614b517f97b293", "20b45dc67b241199", "32ab9031106698ec",
                "cf29b5313fe08a58", "1f675cb3e198b508", "7aa36ce540c3edce", "e13d04180b092d79",
                "e9470edca47de2f5", "8a08ca5f7984087c", "04dd55125dfae325", "04dd55125dfae325",
                "21573b0b90946d2b", "49d640a6a2ffb804", "37982c9962050361", "7479c4483a951450",
                "e244b97bc4c2100d", "e53a5413e9a7aa22", "04dd55125dfae325", "04dd55125dfae325",
                "75d9bb72e24a0b57", "64c1d48eff2650d5", "c9348d834d1da176", "6c69273df5475459",
                "7abd3441a3e054a8", "63a7341bf59394ca", "04dd55125dfae325", "04dd55125dfae325",
                "19c6ef21b7743493", "7b3a3326ac4f154b", "dd362a5c885d0d2a", "8a7945417744498e",
                "21d28a53c26caee5", "73b349d1e7d5df4d", "7234c0f0ac488c9e", "04dd55125dfae325",
                "b16b38a5bff3215a", "24be2ed0599d6d52", "8b57cd55f7405535", "26b215e6bc38344a",
                "04dd55125dfae325", "04dd55125dfae325", "04dd55125dfae325", "04dd55125dfae325",
                "8446c567c42fb6a2", "33d0a354818efade", "0fad0aa0f7fe5bd6", "0774c4e3b5076a78",
                "fe3b108de600324b", "3fac93d1de91709c", "c89821f8c2c9b6bc", "56be632995528474"
            ]
        ),
        Asset(
            name: "built-in fat-mochi",
            atlas: "5f3d445840b6dfb0",
            extensionAtlas: nil,
            frames: [
                "0234b640e70973d4", "c9d00fe11c048a68", "d53165385453df7e", "46a0dfc147d12772",
                "d53165385453df7e", "c9d00fe11c048a68", "0234b640e70973d4", "0234b640e70973d4",
                "1cc2d614c5d2ce30", "b1eabbdaf587d91a", "a88c669ae3f63994", "bdfce512e45f989f",
                "fd9c7f43ca83d0cb", "fd5576e5042d1e11", "937154af5f1d6c91", "bc1b59198e514496",
                "0234b640e70973d4", "743d41ca1a4a12aa", "3cf3a8ad50bf7464", "4eb022ab805937ab",
                "3fa187cf93e75b13", "1da5a8e613acd56d", "3c430e20c9e44389", "e05d689129cc5692",
                "4c7c1cfec5349f27", "800cf42cc6d2e4c5", "966e9d11d6e1f650", "6a701bdf71698845",
                "966e9d11d6e1f650", "800cf42cc6d2e4c5", "4c7c1cfec5349f27", "800cf42cc6d2e4c5",
                "0234b640e70973d4", "311b4fd418c8fa56", "3431851b565dfad9", "a71f47d2df2c491d",
                "e42214f1da5ef3ef", "40a2bca7001c56b4", "5611ad8bbdda5183", "c203d8eb8c3edd2d",
                "0234b640e70973d4", "0a1fa1f3ed4dca17", "b176c4f8186ebba7", "45120fe4700422eb",
                "7e6ee98adaa40f3a", "0234b640e70973d4", "0234b640e70973d4", "0234b640e70973d4",
                "1a0d1ea5cbd935cb", "444d4e97c6234ee2", "1f52f44e2e2c8dd9", "2ec20e91517284e9",
                "0234b640e70973d4", "0234b640e70973d4", "0234b640e70973d4", "0234b640e70973d4"
            ]
        ),
        Asset(
            name: "placeholder",
            atlas: "80db3087cb44b015",
            extensionAtlas: nil,
            frames: [
                "47b1a998f6286914", "f8d695135f61f047", "47b1a998f6286914", "21eebcf42a2cd14a",
                "22eaf2e5cab64470", "77b1c53a05e7a623", "04dd55125dfae325", "04dd55125dfae325",
                "5c7447c0b203a1c6", "fc82ff52a13355c5", "5c7447c0b203a1c6", "3c090e39c6825428",
                "5ae84fb61daee876", "c9383b0ad2582f44", "7cc519d7dbc15656", "d8215acb3378fdbd",
                "5c7447c0b203a1c6", "fc82ff52a13355c5", "5c7447c0b203a1c6", "3c090e39c6825428",
                "5ae84fb61daee876", "c9383b0ad2582f44", "7cc519d7dbc15656", "d8215acb3378fdbd",
                "a342bc60fdf0a0cb", "069d8168a45d6487", "7696ed93f2345ed4", "bcddb65025f8887b",
                "04dd55125dfae325", "04dd55125dfae325", "04dd55125dfae325", "04dd55125dfae325",
                "63a1a6f84f39171d", "dac85e3a047987e0", "5b369fa70cc4df8c", "b711e2c4ea29f6ab",
                "f2714ada631a2f8d", "04dd55125dfae325", "04dd55125dfae325", "04dd55125dfae325",
                "6b4e1e5fa790fcf2", "1f04f7bf6f0802fa", "6b4e1e5fa790fcf2", "91fb7dc71851444b",
                "8e504aff182a4f7a", "7cfa698d7029be6d", "beec501dae56c8f2", "fbe2b1d6099ac8f9",
                "d7971cceae5a36b6", "18ec955c53a5773f", "9879086b5224cd3e", "4cec55445fe0bcd3",
                "8a47cb6af5ee4e7a", "f0aba94479962a04", "04dd55125dfae325", "04dd55125dfae325",
                "2327d0d7fc59b6b8", "2d899a8b96d272e3", "2327d0d7fc59b6b8", "bcd6716505091c42",
                "bdf791dcab90e5ad", "f43597565719db4c", "04dd55125dfae325", "04dd55125dfae325",
                "41b2812b19022588", "e24d6636e2036dec", "41b2812b19022588", "1c934594d0e58f4f",
                "c20c031bee33e60c", "1a267e2d6b7f5070", "04dd55125dfae325", "04dd55125dfae325",
                "ab3f23e815c888ff", "90a2cc758c9592f9", "24c9f0d00f662d7b", "58d1c4c359dfc792",
                "0e035b4b3025577f", "ebf464d7bfc51475", "edfd0929dde0154f", "d4fd3bf315d4f12f",
                "b9b381c55958ab3b", "8c27eac298357631", "7fb177e6f9562f17", "50499c410a7b8206",
                "bf4eac9e86ecc77f", "0a2ef9ceb067af21", "70159a1ad06d9094", "f95e4e160ddfa99f"
            ]
        ),
        Asset(
            name: "package mochi-v3",
            atlas: "099042fa23b1bd92",
            extensionAtlas: "b91016c252ce3218",
            frames: [
                "79535897cbad907a", "f807e227ded1c42a", "0e0c81eb75db0747", "8472b156b5882f3d",
                "8e492dfae3aeda51", "044653465838420a", "04dd55125dfae325", "04dd55125dfae325",
                "430ae8cdecf662da", "3f4f2fcb3302d7a3", "0ac92c30c96f6ba9", "dd24e2622353c41b",
                "e4c6ae6a3e17cb40", "d145c81daab2854b", "68e4637436c4ad9d", "609cc0e013aff30e",
                "fe6e6f5225bc3012", "14ce33fc9bc45893", "a32a3bcf6978a7bd", "6fe64b6ea85fc783",
                "85865ccf512a0aec", "c9e8529cb555a9c3", "589ae570baa04ec5", "d65efeaab4e85a6e",
                "f98a3682f13886e9", "f85266cb8682098f", "7666c2ff22e7c7ed", "f98a3682f13886e9",
                "04dd55125dfae325", "04dd55125dfae325", "04dd55125dfae325", "04dd55125dfae325",
                "08f68a5133d668b5", "f40eac7c22a04a7a", "11eb5804d3ee28d6", "d3e32ea10d819a5b",
                "9b700af78d63f1e8", "04dd55125dfae325", "04dd55125dfae325", "04dd55125dfae325",
                "c642da2cc34e2201", "962e25311ef7dc02", "541500c9ad85a1ad", "3297964d5c250f95",
                "44ce495721a35bba", "67614b517f97b293", "20b45dc67b241199", "32ab9031106698ec",
                "cf29b5313fe08a58", "1f675cb3e198b508", "7aa36ce540c3edce", "e13d04180b092d79",
                "e9470edca47de2f5", "8a08ca5f7984087c", "04dd55125dfae325", "04dd55125dfae325",
                "21573b0b90946d2b", "49d640a6a2ffb804", "37982c9962050361", "7479c4483a951450",
                "e244b97bc4c2100d", "e53a5413e9a7aa22", "04dd55125dfae325", "04dd55125dfae325",
                "75d9bb72e24a0b57", "64c1d48eff2650d5", "c9348d834d1da176", "6c69273df5475459",
                "7abd3441a3e054a8", "63a7341bf59394ca", "04dd55125dfae325", "04dd55125dfae325",
                "19c6ef21b7743493", "7b3a3326ac4f154b", "dd362a5c885d0d2a", "8a7945417744498e",
                "21d28a53c26caee5", "73b349d1e7d5df4d", "7234c0f0ac488c9e", "04dd55125dfae325",
                "b16b38a5bff3215a", "24be2ed0599d6d52", "8b57cd55f7405535", "26b215e6bc38344a",
                "04dd55125dfae325", "04dd55125dfae325", "04dd55125dfae325", "04dd55125dfae325",
                "8446c567c42fb6a2", "33d0a354818efade", "0fad0aa0f7fe5bd6", "0774c4e3b5076a78",
                "fe3b108de600324b", "3fac93d1de91709c", "c89821f8c2c9b6bc", "56be632995528474"
            ]
        )
    ]
}
