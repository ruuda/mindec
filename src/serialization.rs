// Musium -- Music playback daemon with web-based library browser
// Copyright 2020 Ruud van Asseldonk
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// A copy of the License has been included in the root of the repository.

//! Logic for serializing various elements of the index and queue to json.

use serde_json;

use std::io;
use std::io::Write;

use crate::{Album, AlbumId, Artist, ArtistId, MetaIndex, TrackId};
use crate::player::{Millibel, TrackSnapshot};

/// Write an album, but only with the album details, not its tracks.
///
/// Used for the list of all albums, and for the list of albums by artist.
pub fn write_brief_album_json<W: Write>(
    index: &dyn MetaIndex,
    mut w: W,
    album_id: AlbumId,
    album: &Album,
) -> io::Result<()> {
    // The unwrap is safe here, in the sense that if the index is
    // well-formed, it will never fail. The id is provided by the index
    // itself, not user input, so the artist should be present.
    let artist = index.get_artist(album.artist_id).unwrap();

    write!(w, r#"{{"id":"{}","title":"#, album_id)?;
    serde_json::to_writer(&mut w, index.get_string(album.title))?;
    write!(w, r#","artist_id":"{}","artist":"#, album.artist_id)?;
    serde_json::to_writer(&mut w, index.get_string(artist.name))?;
    write!(w, r#","sort_artist":"#)?;
    serde_json::to_writer(&mut w, index.get_string(artist.name_for_sort))?;
    write!(w, r#","date":"{}"}}"#, album.original_release_date)?;
    Ok(())
}

/// Write a json representation of the album list to the writer.
pub fn write_albums_json<W: Write>(index: &dyn MetaIndex, mut w: W) -> io::Result<()> {
    write!(w, "[")?;
    let mut first = true;
    for &(id, ref album) in index.get_albums() {
        if !first { write!(w, ",")?; }
        write_brief_album_json(index, &mut w, id, album)?;
        first = false;
    }
    write!(w, "]")
}

/// Write a json representation of the album and its tracks to the writer.
///
/// The album is expected to come from this index, so the artists and
/// strings it references are valid.
pub fn write_album_json<W: Write>(index: &dyn MetaIndex, mut w: W, id: AlbumId, album: &Album) -> io::Result<()> {
    // The unwrap is safe here, in the sense that if the index is
    // well-formed, it will never fail. The id is provided by the index
    // itself, not user input, so the artist should be present.
    let artist = index.get_artist(album.artist_id).unwrap();

    write!(w, r#"{{"title":"#)?;
    serde_json::to_writer(&mut w, index.get_string(album.title))?;
    write!(w, r#","artist":"#)?;
    serde_json::to_writer(&mut w, index.get_string(artist.name))?;
    write!(w, r#","sort_artist":"#)?;
    serde_json::to_writer(&mut w, index.get_string(artist.name_for_sort))?;
    write!(w, r#","date":"{}","tracks":["#, album.original_release_date)?;
    let mut first = true;
    for &(ref tid, ref track) in index.get_album_tracks(id) {
        if !first { write!(w, ",")?; }
        write!(w, r#"{{"id":"{}","disc_number":{},"track_number":{},"title":"#,
               tid, track.disc_number, track.track_number)?;
        serde_json::to_writer(&mut w, index.get_string(track.title))?;
        write!(w, r#","artist":"#)?;
        serde_json::to_writer(&mut w, index.get_string(track.artist))?;
        write!(w, r#","duration_seconds":{}}}"#, track.duration_seconds)?;
        first = false;
    }
    write!(w, "]}}")
}

/// Write a json representation of the artist and its albums.
pub fn write_artist_json<W: Write>(
    index: &dyn MetaIndex,
    mut w: W,
    artist: &Artist,
    albums: &[(ArtistId, AlbumId)],
) -> io::Result<()> {
    write!(w, r#"{{"name":"#)?;
    serde_json::to_writer(&mut w, index.get_string(artist.name))?;
    write!(w, r#","albums":["#)?;
    let mut first = true;
    for &(_, album_id) in albums {
        // The unwrap is safe here, in the sense that if the index is
        // well-formed, it will never fail. The id is provided by the index
        // itself, not user input, so the album should be present.
        let album = index.get_album(album_id).unwrap();
        if !first { write!(w, ",")?; }
        write_brief_album_json(index, &mut w, album_id, album)?;
        first = false;
    }
    write!(w, "]}}")
}

pub fn write_search_results_json<W: Write>(
    index: &dyn MetaIndex,
    mut w: W,
    artists: &[ArtistId],
    albums: &[AlbumId],
    tracks: &[TrackId],
) -> io::Result<()> {
    write!(w, r#"{{"artists":["#)?;
    let mut first = true;
    for &aid in artists {
        if !first { write!(w, ",")?; }
        write_search_artist_json(index, &mut w, aid)?;
        first = false;
    }
    write!(w, r#"],"albums":["#)?;
    let mut first = true;
    for &aid in albums {
        if !first { write!(w, ",")?; }
        write_search_album_json(index, &mut w, aid)?;
        first = false;
    }
    write!(w, r#"],"tracks":["#)?;
    let mut first = true;
    for &tid in tracks {
        if !first { write!(w, ",")?; }
        write_search_track_json(index, &mut w, tid)?;
        first = false;
    }
    write!(w, r#"]}}"#)
}

pub fn write_search_artist_json<W: Write>(index: &dyn MetaIndex, mut w: W, id: ArtistId) -> io::Result<()> {
    let artist = index.get_artist(id).unwrap();
    let albums = index.get_albums_by_artist(id);
    write!(w, r#"{{"id":"{}","name":"#, id)?;
    serde_json::to_writer(&mut w, index.get_string(artist.name))?;
    write!(w, r#","albums":["#)?;
    let mut first = true;
    for &(_artist_id, album_id) in albums {
        if !first { write!(w, ",")?; }
        write!(w, r#""{}""#, album_id)?;
        first = false;
    }
    write!(w, r#"]}}"#)
}

pub fn write_search_album_json<W: Write>(index: &dyn MetaIndex, mut w: W, id: AlbumId) -> io::Result<()> {
    let album = index.get_album(id).unwrap();
    let artist = index.get_artist(album.artist_id).unwrap();
    write!(w, r#"{{"id":"{}","title":"#, id)?;
    serde_json::to_writer(&mut w, index.get_string(album.title))?;
    write!(w, r#","artist":"#)?;
    serde_json::to_writer(&mut w, index.get_string(artist.name))?;
    write!(w, r#","date":"{}"}}"#, album.original_release_date)
}

pub fn write_search_track_json<W: Write>(index: &dyn MetaIndex, mut w: W, id: TrackId) -> io::Result<()> {
    let track = index.get_track(id).unwrap();
    let album = index.get_album(track.album_id).unwrap();
    write!(w, r#"{{"id":"{}","title":"#, id)?;
    serde_json::to_writer(&mut w, index.get_string(track.title))?;
    write!(w, r#","album_id":"{}","album":"#, track.album_id)?;
    serde_json::to_writer(&mut w, index.get_string(album.title))?;
    write!(w, r#","artist":"#)?;
    serde_json::to_writer(&mut w, index.get_string(track.artist))?;
    write!(w, r#"}}"#)
}

fn write_queued_track_json<W: Write>(
    index: &dyn MetaIndex,
    mut w: W,
    queued_track: &TrackSnapshot,
) -> io::Result<()> {
    // Same as the search result track format, but additionally includes
    // the duration, and playback information.
    let track = index.get_track(queued_track.track_id).unwrap();
    let album = index.get_album(track.album_id).unwrap();
    write!(
        w,
        r#"{{"queue_id":"{}","track_id":"{}","title":"#,
        queued_track.queue_id,
        queued_track.track_id,
    )?;
    serde_json::to_writer(&mut w, index.get_string(track.title))?;
    write!(w, r#","album_id":"{}","album":"#, track.album_id)?;
    serde_json::to_writer(&mut w, index.get_string(album.title))?;
    write!(w, r#","artist":"#)?;
    serde_json::to_writer(&mut w, index.get_string(track.artist))?;
    write!(w, r#","duration_seconds":{}"#, track.duration_seconds)?;

    let position_seconds = queued_track.position_ms as f32 * 1e-3;
    let buffered_seconds = queued_track.buffered_ms as f32 * 1e-3;
    write!(w, r#","position_seconds":{:.03}"#, position_seconds)?;
    write!(w, r#","buffered_seconds":{:.03}"#, buffered_seconds)?;
    write!(w, r#","is_buffering":{}}}"#, queued_track.is_buffering)
}


pub fn write_queue_json<W: Write>(
    index: &dyn MetaIndex,
    mut w: W,
    tracks: &[TrackSnapshot],
) -> io::Result<()> {
    write!(w, "[")?;
    let mut first = true;
    for queued_track in tracks.iter() {
        if !first { write!(w, ",")?; }
        write_queued_track_json(index, &mut w, queued_track)?;
        first = false;
    }
    write!(w, "]")
}

pub fn write_volume_json<W: Write>(mut w: W, current_volume: Millibel) -> io::Result<()> {
    write!(w, r#"{{"volume_db":{:.02}}}"#, current_volume.0 as f32 * 0.01)
}
