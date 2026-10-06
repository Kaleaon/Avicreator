# Avatar development

Avicreator is the primary checkout for avatar work. Source and history were
imported from Kaleaon/Avatarmaker; retain attribution and license notices.
Ktheme remains a separate sibling repository.

Use the existing isolated cloud checkout. Do not create a Git worktree unless
the user explicitly requests one.

- Setup: `bash scripts/setup.sh`.
- Headless tests: `bash scripts/dev.sh test -q`.
- Ingestion API: `bash scripts/dev.sh serve`.
- Python tests mock Blender APIs; they do not establish Blender UI readiness.
- Android uses JDK 17, SDK 34 and Gradle 8.8. Build:
  `cd android && bash gradlew assembleDebug test --max-workers=2`.
- The imported renderer has known Filament API errors documented in README.md.
  Report these separately from environment setup failures.
- Keep generated files, caches, virtual environments and credentials out of
  commits. Do not move or delete sibling repositories during routine work.
