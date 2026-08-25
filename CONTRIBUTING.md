# Contributing to Roamling

Thank you for helping make Roamling feel alive. Contributions should preserve
the project's priorities: cute first, useful second, and never annoying.

## Contributor License Agreement

All code, documentation, and asset contributions require acceptance of the
[Roamling Contributor License Agreement](CLA.md).

The CLA does **not** transfer ownership of your contribution. You keep your
copyright. It gives the project owner enough permission to:

- keep the contribution available under GPL-3.0-only;
- maintain and distribute official Roamling builds; and
- offer separate commercial terms when necessary, without closing the accepted
  contribution in the community version.

Until an automated CLA service is configured, accept the CLA explicitly using
the checkbox in the pull-request template. The GitHub account submitting the
pull request is treated as the electronic signing identity. If you contribute
for an employer or another organization, confirm that you are authorized to do
so before submitting.

## Pull requests

Before opening a pull request:

1. Keep the change focused and explain the behavior it changes.
2. Add or update pure-logic tests where applicable.
3. Run `./scripts/test.sh`.
4. Include screenshots or a short recording for visible behavior changes.
5. Identify every third-party dependency or asset and its license.
6. Do not submit secrets, proprietary source, prompts, or unlicensed pet assets.

AI-assisted contributions are welcome only when the contributor understands
and verifies the result and has the right to submit every included part. An AI
tool does not remove third-party license or attribution obligations.

## License headers

New source files should start with the appropriate comment syntax containing:

```text
SPDX-FileCopyrightText: 2026 Roamling contributors
SPDX-License-Identifier: GPL-3.0-only
```

Third-party files must retain their original notices and must not be relabeled
as Roamling-owned code.
