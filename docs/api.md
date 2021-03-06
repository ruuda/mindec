# API

Endpoints:

 * `GET  /track/:track_id.flac`: Return the track itself.
 * `GET  /album/:album_id`:      Return json album metadata.
 * `GET  /albums`:               Return a json list of all albums.
 * `GET  /artist/:artist_id`:    Return a json object with artist details, and albums in chronological order.
 * `GET  /cover/:album_id`:      Return cover art in original resolution.
 * `GET  /thumb/:album_id`:      Return downsampled cover art.
 * `GET  /search?q=`:            Return json search results.
 * `GET  /queue`:                Return the current play queue.
 * `PUT  /queue/:track_id`:      Enqueue the track with the given id.
 * `GET  /volume`:               Return the current volume.
 * `POST /volume/up`:            Increase the volume by 1 dB.
 * `POST /volume/down`:          Decrease the volume by 1 dB.
