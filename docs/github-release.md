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

macOS 참고: Apple Developer ID / 공증은 아직 없습니다. ad-hoc 서명만으로
Apple Silicon에서 실행은 되지만, 다운로드 후 Gatekeeper 경고가 날 수 있어
첫 실행은 **우클릭 → 열기**가 필요할 수 있습니다. macOS 서명은
`build-dmg.sh`에서만 수행합니다 (`before-each-package-command`에 두면
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
