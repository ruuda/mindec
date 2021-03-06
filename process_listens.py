#!/usr/bin/env python3

# Musium -- Music playback daemon with web-based library browser
# Copyright 2018 Ruud van Asseldonk

# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# A copy of the License has been included in the root of the repository.

"""
process_listens.py -- Convert Listenbrainz json into tab-separated values.

Streaming parsing with Serde is a lot of work for this format, and the Serde
deriving stuff is also not great for this use case. So instead we pre-process
the listens into a tsv file which is a lot easier to parse in Rust. This is not
streaming either, but for now it will do.

Usage:
    process_listens.py listens.json > listens.tsv
"""

import json
import sys

if len(sys.argv) != 2:
    print(__doc__)
    sys.exit(1)

print('seconds_since_epoch\ttrack\tartist\talbum')

for listen in json.load(open(sys.argv[1], 'r', encoding='utf-8')):
    meta = listen['track_metadata']
    if (
        meta['track_name'] is None or
        meta['artist_name'] is None or
        meta['release_name'] is None
    ):
        continue

    line = (
        str(listen['listened_at']) + '\t' +
        meta['track_name'] + '\t' +
        meta['artist_name'] + '\t' +
        meta['release_name']
    )

    # Skip misencodings. Better would be to try and fix them, but for now we
    # just skip them. U+0018 is the Cancel control character, probably something
    # very bad happened to the data that made this appear.
    if 'Ã©' in line or '\u0018' in line or 'â€' in line:
        continue

    print(line)
