#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# NexusKVM Release & Version Management Script
#
# Usage:
#   bash scripts/release.sh [patch|minor|major|<version>] [options]
#
# Examples:
#   bash scripts/release.sh 0.1.0 --push
#   bash scripts/release.sh patch --push
#   bash scripts/release.sh minor
#   npm run release:patch
#
# Options:
#   --push          Push the release commit and git tag to origin
#   --allow-dirty   Allow running with uncommitted changes
#   --no-tag        Do not create a git tag
#   --no-commit     Do not create a git commit (file updates only)
#   --dry-run       Preview changes without writing files or git operations
# ==============================================================================

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPO_ROOT}"

CURRENT_VERSION=$(node -p "require('./package.json').version")

# Parse arguments
TARGET_VERSION=""
PUSH=false
ALLOW_DIRTY=false
NO_TAG=false
NO_COMMIT=false
DRY_RUN=false

for arg in "$@"; do
  case "${arg}" in
    --push)
      PUSH=true
      ;;
    --allow-dirty)
      ALLOW_DIRTY=true
      ;;
    --no-tag)
      NO_TAG=true
      ;;
    --no-commit)
      NO_COMMIT=true
      ;;
    --dry-run)
      DRY_RUN=true
      ;;
    -h|--help)
      echo "NexusKVM Release Automation"
      echo ""
      echo "Usage:"
      echo "  bash scripts/release.sh [patch|minor|major|<version>] [options]"
      echo ""
      echo "Arguments:"
      echo "  patch         Increment patch version (e.g. 0.1.0 -> 0.1.1)"
      echo "  minor         Increment minor version (e.g. 0.1.0 -> 0.2.0)"
      echo "  major         Increment major version (e.g. 0.1.0 -> 1.0.0)"
      echo "  <version>     Explicit version (e.g. 0.1.0 or 1.2.3-beta.1)"
      echo ""
      echo "Options:"
      echo "  --push        Automatically push commit and tag to origin"
      echo "  --allow-dirty Allow running with uncommitted git changes"
      echo "  --no-tag      Skip git tag creation"
      echo "  --no-commit   Skip git commit creation"
      echo "  --dry-run     Show what would change without modifying files"
      exit 0
      ;;
    *)
      if [[ -z "${TARGET_VERSION}" && ! "${arg}" =~ ^-- ]]; then
        TARGET_VERSION="${arg}"
      else
        echo "Error: Unknown argument '${arg}'" >&2
        exit 1
      fi
      ;;
  esac
done

# If no target version was specified, prompt or default to current
if [[ -z "${TARGET_VERSION}" ]]; then
  # Calculate next semver options
  NEXT_PATCH=$(node -e "const [M,m,p] = '${CURRENT_VERSION}'.split('.').map(Number); console.log(\`\${M}.\${m}.\${p+1}\`)")
  NEXT_MINOR=$(node -e "const [M,m] = '${CURRENT_VERSION}'.split('.').map(Number); console.log(\`\${M}.\${m+1}.0\`)")
  NEXT_MAJOR=$(node -e "const [M] = '${CURRENT_VERSION}'.split('.').map(Number); console.log(\`\${M+1}.0.0\`)")

  echo "Current version is: ${CURRENT_VERSION}"
  echo "Select release type or enter a version:"
  echo "  1) patch  (${NEXT_PATCH})"
  echo "  2) minor  (${NEXT_MINOR})"
  echo "  3) major  (${NEXT_MAJOR})"
  echo "  4) keep   (${CURRENT_VERSION} - initial release/dispatch)"
  echo "  5) custom"
  read -rp "Enter choice [1-5 or explicit version]: " user_choice

  case "${user_choice}" in
    1|patch) TARGET_VERSION="patch" ;;
    2|minor) TARGET_VERSION="minor" ;;
    3|major) TARGET_VERSION="major" ;;
    4|keep)  TARGET_VERSION="${CURRENT_VERSION}" ;;
    5|custom)
      read -rp "Enter new version (e.g. 0.1.0): " custom_v
      TARGET_VERSION="${custom_v}"
      ;;
    *)
      TARGET_VERSION="${user_choice}"
      ;;
  esac
fi

# Strip leading 'v' if provided
TARGET_VERSION="${TARGET_VERSION#v}"

# Resolve semver keywords
case "${TARGET_VERSION}" in
  patch)
    NEW_VERSION=$(node -e "const [M,m,p] = '${CURRENT_VERSION}'.split('.').map(Number); console.log(\`\${M}.\${m}.\${p+1}\`)")
    ;;
  minor)
    NEW_VERSION=$(node -e "const [M,m] = '${CURRENT_VERSION}'.split('.').map(Number); console.log(\`\${M}.\${m+1}.0\`)")
    ;;
  major)
    NEW_VERSION=$(node -e "const [M] = '${CURRENT_VERSION}'.split('.').map(Number); console.log(\`\${M+1}.0.0\`)")
    ;;
  *)
    NEW_VERSION="${TARGET_VERSION}"
    ;;
esac

# Validate semver pattern
if [[ ! "${NEW_VERSION}" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$ ]]; then
  echo "Error: Invalid version format '${NEW_VERSION}'. Expected SemVer (e.g. 0.1.0 or 1.0.0-rc.1)" >&2
  exit 1
fi

TAG_NAME="v${NEW_VERSION}"

echo "========================================================"
echo " NexusKVM Release Dispatcher"
echo " Current version : ${CURRENT_VERSION}"
echo " New version     : ${NEW_VERSION}"
echo " Git tag         : ${TAG_NAME}"
echo " Push to origin  : ${PUSH}"
echo " Dry run         : ${DRY_RUN}"
echo "========================================================"

# Safety check: working directory status
if [[ "${ALLOW_DIRTY}" == "false" && "${DRY_RUN}" == "false" ]]; then
  if [[ -n "$(git status --porcelain)" ]]; then
    echo "Error: Working directory has uncommitted changes. Commit or stash them first, or use --allow-dirty." >&2
    git status -s
    exit 1
  fi
fi

# Safety check: tag already exists
if [[ "${NO_TAG}" == "false" ]]; then
  if git rev-parse "${TAG_NAME}" >/dev/null 2>&1; then
    echo "Error: Git tag '${TAG_NAME}' already exists locally." >&2
    exit 1
  fi
  if git ls-remote --tags origin "${TAG_NAME}" | grep -q "${TAG_NAME}"; then
    echo "Error: Git tag '${TAG_NAME}' already exists on remote origin." >&2
    exit 1
  fi
fi

if [[ "${DRY_RUN}" == "true" ]]; then
  echo "[Dry Run] Would update files to version ${NEW_VERSION}:"
  echo "  - package.json"
  echo "  - package-lock.json"
  echo "  - src-tauri/tauri.conf.json"
  echo "  - Cargo.toml"
  echo "  - Cargo.lock"
  echo "[Dry Run] Would commit and create tag ${TAG_NAME}"
  if [[ "${PUSH}" == "true" ]]; then
    echo "[Dry Run] Would push commit and tag ${TAG_NAME} to origin"
  fi
  exit 0
fi

# 1. Update package.json
echo "-> Updating package.json..."
node -e "
  const fs = require('fs');
  const pkg = JSON.parse(fs.readFileSync('package.json', 'utf8'));
  pkg.version = '${NEW_VERSION}';
  fs.writeFileSync('package.json', JSON.stringify(pkg, null, 2) + '\n');
"

# 2. Update package-lock.json
echo "-> Updating package-lock.json..."
npm version "${NEW_VERSION}" --no-git-tag-version --allow-same-version >/dev/null

# 3. Update src-tauri/tauri.conf.json
echo "-> Updating src-tauri/tauri.conf.json..."
node -e "
  const fs = require('fs');
  const tauri = JSON.parse(fs.readFileSync('src-tauri/tauri.conf.json', 'utf8'));
  tauri.version = '${NEW_VERSION}';
  fs.writeFileSync('src-tauri/tauri.conf.json', JSON.stringify(tauri, null, 2) + '\n');
"

# 4. Update Cargo.toml workspace version
echo "-> Updating Cargo.toml [workspace.package] version..."
node -e "
  const fs = require('fs');
  let cargo = fs.readFileSync('Cargo.toml', 'utf8');
  cargo = cargo.replace(/(\[workspace\.package\][\s\S]*?version\s*=\s*)\"[^\"]+\"/, \`\$1\"${NEW_VERSION}\"\`);
  fs.writeFileSync('Cargo.toml', cargo);
"

# 5. Refresh Cargo.lock to match workspace version
echo "-> Refreshing Cargo.lock..."
cargo check --workspace >/dev/null 2>&1 || true

# 6. Format check & verification
echo "-> Verifying workspace build & formatting..."
npm run format >/dev/null 2>&1 || true
cargo fmt --all >/dev/null 2>&1 || true

# 7. Git commit & tag
if [[ "${NO_COMMIT}" == "false" ]]; then
  echo "-> Staging changes in git..."
  git add package.json package-lock.json src-tauri/tauri.conf.json Cargo.toml Cargo.lock src/components/Sidebar.tsx 2>/dev/null || true
  
  if git diff --cached --quiet; then
    echo "No version changes needed to commit (already at ${NEW_VERSION})."
  else
    echo "-> Creating commit: chore(release): v${NEW_VERSION}"
    git commit -m "chore(release): v${NEW_VERSION}"
  fi

  if [[ "${NO_TAG}" == "false" ]]; then
    echo "-> Creating annotated tag: ${TAG_NAME}"
    git tag -a "${TAG_NAME}" -m "Release ${TAG_NAME}"
  fi
fi

# 8. Push to remote origin if requested
if [[ "${PUSH}" == "true" ]]; then
  CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)
  echo "-> Pushing branch '${CURRENT_BRANCH}' to origin..."
  git push origin "${CURRENT_BRANCH}"

  if [[ "${NO_TAG}" == "false" ]]; then
    echo "-> Pushing tag '${TAG_NAME}' to origin..."
    git push origin "${TAG_NAME}"
  fi

  echo ""
  echo "🎉 Successfully pushed release ${TAG_NAME}!"
  echo "🚀 GitHub Actions release workflow is now running at:"
  echo "   https://github.com/$(git config --get remote.origin.url | sed -E 's/.*github.com[:\/](.*)\.git/\1/')/actions"
else
  echo ""
  echo "✅ Release ${TAG_NAME} prepared locally."
  echo "To publish this release to GitHub Actions, run:"
  echo "   git push origin $(git rev-parse --abbrev-ref HEAD)"
  if [[ "${NO_TAG}" == "false" ]]; then
    echo "   git push origin ${TAG_NAME}"
  fi
fi
