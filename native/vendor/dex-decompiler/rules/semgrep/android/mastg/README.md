# OWASP MASTG Semgrep rules (Android)

Vendored from [OWASP MASTG](https://github.com/OWASP/mastg/tree/master/rules) (Android Semgrep rules).

These YAML files are included in the default `--scan-semgrep` rule set together with the general Android rules in `../general.yml`.

## License / attribution

Copyright OWASP Mobile Application Security Testing Guide (MASTG) contributors.
See the upstream repository for license terms (typically Creative Commons / project LICENSE).

Upstream: https://github.com/OWASP/mastg

## Notes

- **Java** rules run on decompiled method bodies (token patterns / `pattern-regex`; Semgrep features like `pattern-not` and `metavariable-regex` are best-effort / ignored).
- **XML** rules run on **plaintext** `AndroidManifest.xml`. APKs store binary AXML; pass a decoded manifest (e.g. apktool) as input, or place a text `AndroidManifest.xml` next to the APK.
