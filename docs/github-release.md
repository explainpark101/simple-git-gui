# GitHub Release 가이드

`main`에 푸시할 때마다 Actions가 루트 `Cargo.toml`의
`[workspace.package] version`을 읽고, 해당 버전의 태그/`Release`가 없을 때만
설치 파일을 빌드해 GitHub Release에 올립니다. **같은 버전으로는 중복
태그·릴리스를 만들지 않습니다.**

## 전체 흐름

1. `Cargo.toml`에서 버전 bump (예: `1.0.0` → `1.1.0`)
2. 커밋 후 `main`에 푸시
3. Actions `Release` 워크플로가 `v1.1.0` 태그 존재 여부 확인
4. 없으면 Windows / macOS / Linux 패키지 빌드
5. 태그 `vX.Y.Z` + GitHub Release 생성 및 자산 업로드

이미 `vX.Y.Z` 태그(또는 동명 Release)가 있으면 워크플로는 성공으로 끝나고
빌드/릴리스는 건너뜁니다.

## 릴리스할 때 내가 할 일

1. 루트 `Cargo.toml`의 `[workspace.package] version`만 올린다.
2. 변경을 커밋하고 `main`에 푸시한다.

```bash
# example: bump version in Cargo.toml, then
git add Cargo.toml Cargo.lock crates/*/Cargo.toml
git commit -m "chore: bump version to 1.1.0"
git push origin main
```

수동으로 태그를 푸시할 필요는 없습니다. (같은 버전을 다시 올리려면
기존 태그/`Release`를 삭제한 뒤 `main`에 재푸시하거나 `workflow_dispatch`로
재실행하세요.)

수동 재실행: GitHub → Actions → **Release** → **Run workflow**.

## 검증

1. GitHub **Actions**에서 `Release` 워크플로가 초록인지 확인
2. GitHub **Releases**에서 `vX.Y.Z`와 자산 확인
   - Windows: `*-setup.exe` (NSIS), `sgg.exe`
   - macOS: `*.dmg` (서명된 `.app` + `Install sgg.command`; PATH에 `sgg` 등록)
   - Linux: `*.AppImage`, `sgg`
   - `latest.json` — 버전 + 플랫폼별 다운로드 URL (앱/CLI 자동 업데이트용).
     고정 URL: `…/releases/latest/download/latest.json`

### macOS 서명 / 공증 (Gatekeeper)

브라우저로 DMG를 받으면 Gatekeeper가 “악성 코드가 없음을 확인할 수 없습니다”를
띄울 수 있습니다. **완전히 없애려면** Apple Developer Program + Developer ID
서명 + 공증이 필요합니다.

CI에 아래 GitHub Actions secrets를 넣으면 `build-dmg.sh`가 Developer ID로
서명하고 `notarytool`로 공증·스테이플합니다.

| Secret | 설명 |
| --- | --- |
| `MACOS_CERTIFICATE` | Developer ID Application `.p12` (base64) |
| `MACOS_CERTIFICATE_PASSWORD` | `.p12` 비밀번호 |
| `MACOS_SIGNING_IDENTITY` | (선택) `Developer ID Application: Name (TEAMID)` |
| `APPLE_API_KEY` | App Store Connect API `.p8` (base64) |
| `APPLE_API_KEY_ID` | 키 ID |
| `APPLE_API_ISSUER` | Issuer UUID |
| 또는 `APPLE_ID` + `APPLE_APP_SPECIFIC_PASSWORD` + `APPLE_TEAM_ID` | API 키 대신 Apple ID 인증 |

secrets가 없으면 기존처럼 **ad-hoc** 서명만 하고 릴리스는 성공합니다.
그 경우 사용자는 Release 본문의 `xattr -cr` 안내 또는
`packaging/macos/install-from-github.sh`로 설치하면 됩니다.

macOS 서명은 `build-dmg.sh` → `packaging/macos-sign.sh` /
`macos-notarize.sh`에서만 수행합니다 (`before-each-package-command`에 두면
Windows NSIS 빌드가 `cmd /C`로 깨집니다).

## 이 프로젝트 설정

- 워크플로: `.github/workflows/release.yml`
- 버전 소스: 루트 `Cargo.toml` → `[workspace.package].version`
- 태그 형식: `v` + 버전 (예: `1.0.0` → `v1.0.0`)
- 빌드: `./build-exe.sh` / `./build-dmg.sh` / `./build-appimage.sh`
- Runner: `windows-latest`, `macos-latest`, `ubuntu-latest`

## 주의

- 웹 UI에서 Release를 직접 만들면 자동화와 겹칠 수 있습니다. 버전 bump +
  `main` 푸시를 권장합니다.
- 새 릴리스를 내려면 **반드시** `Cargo.toml` 버전을 올리세요. 올리지 않고
  `main`만 푸시하면 기존 태그 때문에 스킵됩니다.
