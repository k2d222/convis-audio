#!/bin/sh

#  download all `.node` artifacts from the latest GitHub workflow run, and copy them in the platforms/ folders.


# download all artifacts. They go each in one directory.
gh run download $1 -p '*'

# move the artifact into the platforms/ dir and delete the artifact directory.
for dir in audio-*.node/; do
  triplet="${dir%.node/}"           # audio-<triplet>.node
  triplet="${triplet#audio-}"   # <triplet>.node
  triplet="${triplet%.node}"      # triplet

  src="$dir/audio.node"
  dst="platforms/$triplet/audio-$triplet.node"

  mv "$src" "$dst"
  rmdir "$dir"
done
