#!/bin/sh

set -e

# first command creates the version with git commit, tag etc. in the root package
pnpm version "$@"

# then we amend the version with the bump in sub-packages
pnpm --filter '!@convis/audio-native' exec pnpm version "$@"
git add .
git commit --amend --no-edit

echo "successfully bumped the version."
echo "review the commit then run `git push --follow-tags .`"
echo "this will trigger the workflow to build and publish the release to npm."
