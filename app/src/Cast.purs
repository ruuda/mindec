-- Mindec -- Music metadata indexer
-- Copyright 2019 Ruud van Asseldonk
--
-- Licensed under the Apache License, Version 2.0 (the "License");
-- you may not use this file except in compliance with the License.
-- A copy of the License has been included in the root of the repository.

module Cast
  ( MusicTrackMetadata
  , makeQueueItem
  , QueueItem
  , playTrack
  , queueTrack
  ) where

import Effect (Effect)
import Prelude

type MusicTrackMetadata =
  { discNumber  :: Int
  , trackNumber :: Int
  , title       :: String
  , artist      :: String
  , albumTitle  :: String
  , albumArtist :: String
  , releaseDate :: String
  , imageUrl    :: String
  , trackUrl    :: String
  }

foreign import data QueueItem :: Type

foreign import makeQueueItem :: MusicTrackMetadata -> QueueItem
foreign import playTrack :: MusicTrackMetadata -> Effect Unit
foreign import queueTrack :: QueueItem -> Effect Unit