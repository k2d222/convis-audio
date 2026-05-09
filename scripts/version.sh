#!/bin/sh

set -e

# first command creates the version with git commit, tag etc. in the root package
pnpm version "$@"

# then we amend the version with the bump in sub-packages
pnpm --filter '!@convis/audio-native' exec pnpm version "$@"
git add .
git commit --amend --no-edit
