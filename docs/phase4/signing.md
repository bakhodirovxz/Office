# Code signing (Phase 4 WP13, F11)

Two paths. Pilot: the organisation's internal (AD CS) code-signing certificate, signed locally with `scripts/sign.ps1`.
Public releases (Phase 10): SignPath Foundation (free for open source), signed from a CI build of the public repo.

## 1. The local script

```powershell
scripts/sign.ps1 -Thumbprint <sha1> [-Store CurrentUser|LocalMachine] [-TimestampUrl <RFC3161 url>] <file> [<file> ...]
```

For each file it runs `signtool sign /fd sha256 /tr <url> /td sha256 /sha1 <thumbprint> /s My`, then
`signtool verify /pa /v`. Exit 0 = all signed and verified, 1 = a sign or verify failed, 2 = usage or no signtool.
A hardware token or HSM works the same way when its minidriver puts the certificate in the store (signtool asks for the PIN).
Sign every exe first, then build the MSI from the signed exes, then sign the MSI.

signtool comes from the Windows SDK (`C:\Program Files (x86)\Windows Kits\10\bin\<ver>\x64`, installed by WP13, see `msvc.md`).

Timestamp server: default `http://timestamp.sectigo.com`. From the owner's PC `timestamp.digicert.com`, `globalsign` and
`certum` timed out on 2026-10-22 and Sectigo answered. An offline PC cannot sign with a timestamp; use an internal RFC 3161 server.

### Test result (2026-10-22)
A self-signed `CodeSigningCert` was created in `Cert:\CurrentUser\My` (never in a machine store), used to sign a copy of
`convert.exe` with the Sectigo timestamp, then deleted. The signature and the timestamp (Sectigo Public Time Stamping
Signer R37) were applied. `verify /pa` correctly **failed** with "terminated in a root certificate which is not trusted":
the test certificate is not in any Root store. I did not add it to a trusted store (that weakens the PC's trust);
the `verify` pass is proven only with a real chain, see section 2.

## 2. Pilot: internal enterprise-CA certificate (for the owner's IT)

1. **Template.** On the AD CS issuing CA, duplicate the *Code Signing* template (v2/v3, name e.g. `OfficeSuiteCodeSigning`).
   Key: RSA 3072 (or 2048 minimum), SHA-256, *private key not exportable*. Enrolment limited to a signers' security group.
   EKU must contain *Code Signing* (1.3.6.1.5.5.7.3.3). Publish the template on the CA.
2. **Enrol.** The signing machine or the person enrols (`certmgr.msc` > Personal > Request New Certificate, or
   `certreq`). The key stays on that machine; better, on a smart card/token or an HSM-backed KSP. List the thumbprint:
   `Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert | ft Subject,Thumbprint,NotAfter`.
3. **Sign.** `scripts/sign.ps1 -Thumbprint <that> <exe...>`; the build machine needs only the SDK, not the key.
4. **Trust by GPO** (so `verify /pa`, SmartScreen-less launch and AppLocker/WDAC see it as trusted). Domain machines trust
   the enterprise root already through AD (`NTAuth` + Root autoenrolment). If the issuing CA is not already in these stores:
   - *Computer Configuration > Policies > Windows Settings > Security Settings > Public Key Policies > Trusted Root
     Certification Authorities*: import the root CA certificate (and *Intermediate Certification Authorities* for a sub-CA).
   - *Trusted Publishers*: import the **signing certificate itself** (`.cer`, public part only). This is what stops the
     "Unknown publisher" prompt and satisfies "allow only trusted publishers" policies.
   - Check on a client: `gpupdate /force`, then `signtool verify /pa /v <exe>` and `certutil -store -enterprise Root`.
5. **Deploy.** The MSI is pushed by GPO software installation or the management tool; a pushed MSI carries no
   mark-of-the-web, so SmartScreen does not prompt. Renew before `NotAfter`; the RFC 3161 timestamp keeps old signatures valid.
6. Revocation: the CA's CRL/OCSP endpoint must be reachable from client PCs.

## 3. Public releases: SignPath Foundation (Phase 10)

SignPath Foundation signs open-source releases for free with a certificate issued to SignPath Foundation. The key stays in
SignPath's HSM; we never hold a private key. Requirements and how we meet them:

- **Public repo and public CI.** Signing is submitted from a GitHub Actions run of the public repo. Nothing local is signed
  for release. Only publish through `E:/office-publish/publish.sh` (sanitised), and only after the owner's OK.
- **OSI-approved licence.** MIT OR Apache-2.0: fine. The MPL-2.0 symphonia crates are linked, unmodified, and listed in NOTICE.
- **Built from source in CI, reproducible enough to attest.** Artefacts to sign are uploaded with `actions/upload-artifact`
  in the same run; SignPath reads them back and checks the origin (repo, workflow, commit) against the project's trusted build system.
- **Code-signing policy page.** SignPath requires a public page (README section or `docs/code-signing-policy.md`) stating:
  who the roles are (committers/reviewers, approvers: the owner), that every release is signed only from the public
  repo's CI, that only the product's own binaries are signed (no upstream/third-party binaries are re-signed), the privacy
  statement, and the line "Free code signing provided by SignPath.io, certificate by SignPath Foundation". Write it before applying.
- **Approval.** The release signing policy should require a manual approval by the owner in SignPath.
- **No product name yet** (ADR Appendix C): apply for the SignPath project only after the name is chosen; the certificate
  subject shows SignPath Foundation, but the project/publisher info uses our name.
- **Application** is the owner's item (SignPath Foundation account, project `office-suite`, signing policies `test-signing`
  and `release-signing`, secrets `SIGNPATH_API_TOKEN` and variable `SIGNPATH_ORGANIZATION_ID` on the GitHub repo).

Workflow: `.github/workflows/ci.yml` has two stub jobs, both dispatch-only and off by default. `msvc` (input `msvc`) builds the
exes on `windows-latest` and uploads `msvc-unsigned`; `signpath` (inputs `msvc` and `signpath_submit`) submits that artefact
with `signpath/github-action-submit-signing-request` (pinned to a commit SHA, v2.3) and uploads `msvc-signed`. The MSI is
built and submitted the same way once WP14 exists (a second submission, or one artefact zip holding the exes and the MSI,
with an artifact configuration in the SignPath project). Not tested: no remote, no SignPath project. Verify the
action's inputs against its README when enabling.

## 4. Other platforms (brief)

- **Linux:** packages and AppImage signed with our own GPG key (detached `.asc`, key fingerprint published in the repo).
- **Android:** our own upload/app-signing key (keystore held by the owner, never in CI logs). Android is deferred.
- **Apple (macOS/iOS):** out of scope (no Mac, no developer account).
