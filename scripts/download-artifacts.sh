#!/bin/sh

#  download all `.node` artifacts from the latest GitHub workflow run, and copy them in the platforms/ folders.


# download all artifacts. They go each in one directory.
gh run download -p '*'

# move the artifact into the platforms/ dir and delete the artifact directory.
for dir in audio-*.node/; do
  filename="${dir%/}"           # audio-<triplet>.node
  triplet="${filename#audio-}"   # <triplet>.node
  triplet="${triplet%.node}"      # triplet

  src="$dir/$triplet.node"
  dst="platforms/$triplet/$name"

  mv "$src" "$dst"
  rmdir "$dir"
done
