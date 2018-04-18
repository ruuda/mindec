// Mindec -- Music metadata indexer
// Copyright 2018 Ruud van Asseldonk

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// A copy of the License has been included in the root of the repository.

extern crate claxon;
extern crate futures;
extern crate hyper;
extern crate mdns;
extern crate mindec;
extern crate serde_json;
extern crate walkdir;

use std::env;
use std::time::{Duration, SystemTime};
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process;
use std::rc::Rc;

use futures::future::Future;
use hyper::header::{AccessControlAllowOrigin, ContentLength, ContentType, Expires, HttpDate};
use hyper::mime;
use hyper::server::{Http, Request, Response, Service};
use hyper::{Get, StatusCode};
use mindec::{AlbumId, MetaIndex, MemoryMetaIndex, TrackId};

struct MetaServer {
    index: MemoryMetaIndex,
    cache_dir: PathBuf,
}

type BoxFuture = Box<Future<Item=Response, Error=hyper::Error>>;

impl MetaServer {
    fn new(index: MemoryMetaIndex, cache_dir: &str) -> MetaServer {
        MetaServer {
            index: index,
            cache_dir: PathBuf::from(cache_dir),
        }
    }

    fn handle_not_found(&self) -> BoxFuture {
        let not_found = "Not Found";
        let response = Response::new()
            .with_status(StatusCode::NotFound)
            .with_header(ContentLength(not_found.len() as u64))
            .with_body(not_found);
        Box::new(futures::future::ok(response))
    }

    fn handle_bad_request(&self, reason: &'static str) -> BoxFuture {
        let response = Response::new()
            .with_status(StatusCode::BadRequest)
            .with_header(ContentLength(reason.len() as u64))
            .with_body(reason);
        Box::new(futures::future::ok(response))
    }

    fn handle_error(&self, reason: &'static str) -> BoxFuture {
        let response = Response::new()
            .with_status(StatusCode::InternalServerError)
            .with_header(ContentLength(reason.len() as u64))
            .with_body(reason);
        Box::new(futures::future::ok(response))
    }

    fn handle_track_cover(&self, _request: &Request, id: &str) -> BoxFuture {
        // TODO: DRY this track id parsing and loadong part.
        let track_id = match TrackId::parse(id) {
            Some(tid) => tid,
            None => return self.handle_bad_request("Invalid track id."),
        };

        let track = match self.index.get_track(track_id) {
            Some(t) => t,
            None => return self.handle_not_found(),
        };

        let fname = self.index.get_filename(track.filename);

        let opts = claxon::FlacReaderOptions {
            metadata_only: true,
            read_picture: claxon::ReadPicture::CoverAsVec,
            read_vorbis_comment: false,
        };
        let reader = match claxon::FlacReader::open_ext(fname, opts) {
            Ok(r) => r,
            Err(..) => return self.handle_error("Failed to open flac file."),
        };

        if let Some(cover) = reader.into_pictures().pop() {
            let mime = match cover.mime_type.parse::<mime::Mime>() {
                Ok(m) => m,
                Err(..) => {
                    // TODO: Add a proper logging mechanism.
                    println!("Warning invalid mime type: '{}' in track {} ({}).", cover.mime_type, id, fname);
                    return self.handle_error("Invalid mime type.")
                }
            };
            let data = cover.into_vec();
            let expires = SystemTime::now() + Duration::from_secs(3600 * 24 * 30);
            let response = Response::new()
                .with_header(AccessControlAllowOrigin::Any)
                .with_header(Expires(HttpDate::from(expires)))
                .with_header(ContentType(mime))
                .with_header(ContentLength(data.len() as u64))
                .with_body(data);
            Box::new(futures::future::ok(response))
        } else {
            // The file has no embedded front cover.
            self.handle_not_found()
        }
    }

    fn handle_thumb(&self, _request: &Request, id: &str) -> BoxFuture {
        // TODO: DRY this track id parsing and loadong part.
        let album_id = match AlbumId::parse(id) {
            Some(aid) => aid,
            None => return self.handle_bad_request("Invalid album id."),
        };

        let mut fname: PathBuf = PathBuf::from(&self.cache_dir);
        fname.push(format!("{}.jpg", album_id));
        let mut file = match fs::File::open(fname) {
            Ok(f) => f,
            // TODO: This is not entirely accurate. Also, try to generate the
            // thumbnail if it does not exist.
            Err(..) => return self.handle_not_found(),
        };
        let mut data = Vec::new();
        match file.read_to_end(&mut data) {
            Ok(..) => {}
            Err(..) => return self.handle_error("Failed to read cached thumbnail."),
        }
        let expires = SystemTime::now() + Duration::from_secs(3600 * 24 * 30);
        let mime = "image/jpeg".parse::<mime::Mime>().unwrap();
        let response = Response::new()
            .with_header(AccessControlAllowOrigin::Any)
            .with_header(Expires(HttpDate::from(expires)))
            .with_header(ContentType(mime))
            .with_header(ContentLength(data.len() as u64))
            .with_body(data);
        Box::new(futures::future::ok(response))
    }

    fn handle_track(&self, _request: &Request, path: &str) -> BoxFuture {
        // Track urls are of the form `/track/f7c153f2b16dc101.flac`.
        if !path.ends_with(".flac") {
            return self.handle_bad_request("Expected a path ending in .flac.")
        }

        let id_part = &path[..path.len() - ".flac".len()];
        let track_id = match TrackId::parse(id_part) {
            Some(tid) => tid,
            None => return self.handle_bad_request("Invalid track id."),
        };

        let track = match self.index.get_track(track_id) {
            Some(t) => t,
            None => return self.handle_not_found(),
        };

        let fname = self.index.get_filename(track.filename);

        // TODO: Rather than reading the file into memory in userspace, use
        // sendfile. Hyper seems to be over-engineered for my use case, just
        // writing to a TCP socket would be simpler.
        let mut file = match fs::File::open(fname) {
            Ok(f) => f,
            Err(_) => return self.handle_error("Failed to open file."),
        };
        let len_hint = file.metadata().map(|m| m.len()).unwrap_or(4096);
        let mut body = Vec::with_capacity(len_hint as usize);
        if let Err(_) = file.read_to_end(&mut body) {
            return self.handle_error("Failed to read file.")
        }

        // TODO: Handle requests with Range header.
        let audio_flac = "audio/flac".parse::<mime::Mime>().unwrap();
        let response = Response::new()
            .with_header(AccessControlAllowOrigin::Any)
            .with_header(ContentType(audio_flac))
            .with_header(ContentLength(body.len() as u64))
            .with_body(body);
        Box::new(futures::future::ok(response))
    }

    fn handle_album(&self, _request: &Request, id: &str) -> BoxFuture {
        let album_id = match AlbumId::parse(id) {
            Some(aid) => aid,
            None => return self.handle_bad_request("Invalid album id."),
        };

        let album = match self.index.get_album(album_id) {
            Some(a) => a,
            None => return self.handle_not_found(),
        };

        let buffer = Vec::new();
        let mut w = io::Cursor::new(buffer);
        self.index.write_album_json(&mut w, album_id, album).unwrap();
        let response = Response::new()
            .with_header(ContentType::json())
            .with_header(AccessControlAllowOrigin::Any)
            .with_body(w.into_inner());
        Box::new(futures::future::ok(response))
    }

    fn handle_albums(&self, _request: &Request) -> BoxFuture {
        let buffer = Vec::new();
        let mut w = io::Cursor::new(buffer);
        self.index.write_albums_json(&mut w).unwrap();
        let response = Response::new()
            .with_header(ContentType::json())
            .with_header(AccessControlAllowOrigin::Any)
            .with_body(w.into_inner());
        Box::new(futures::future::ok(response))
    }

    fn handle_artist(&self, _request: &Request, _id: &str) -> BoxFuture {
        let response = Response::new().with_body("Artist");
        Box::new(futures::future::ok(response))
    }

    fn handle_search(&self, _request: &Request) -> BoxFuture {
        let response = Response::new().with_body("Search");
        Box::new(futures::future::ok(response))
    }
}

impl Service for MetaServer {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = BoxFuture;

    fn call(&self, request: Request) -> Self::Future {
        println!("Request: {:?}", request);

        let mut parts = request
            .uri()
            .path()
            .splitn(3, '/')
            .filter(|x| x.len() > 0);

        let p0 = parts.next();
        let p1 = parts.next();

        // A very basic router. See also docs/api.md for an overview.
        match (request.method(), p0, p1) {
            (&Get, Some("cover"),  Some(t)) => self.handle_track_cover(&request, t),
            (&Get, Some("thumb"),  Some(t)) => self.handle_thumb(&request, t),
            (&Get, Some("track"),  Some(t)) => self.handle_track(&request, t),
            (&Get, Some("album"),  Some(a)) => self.handle_album(&request, a),
            (&Get, Some("albums"), None)    => self.handle_albums(&request),
            (&Get, Some("artist"), Some(a)) => self.handle_artist(&request, a),
            (&Get, Some("search"), None)    => self.handle_search(&request),
            (&Get, _, _) => self.handle_not_found(),
            _ => self.handle_bad_request("Expected a GET request."),
        }
    }
}

fn make_index(dir: &str) -> MemoryMetaIndex {
    let wd = walkdir::WalkDir::new(&dir)
        .follow_links(true)
        .max_open(128);

    let flac_ext = OsStr::new("flac");

    let index;
    {
        let stdout = std::io::stdout();
        let mut lock = stdout.lock();

        // First enumerate all flac files, before indexing them. It turns out
        // that this is faster than indexing them on the go (and not first
        // collecting into a vector). See also performance.md in the root of the
        // repository.
        let mut k = 0;
        let mut paths = Vec::new();
        let paths_iter = wd
            .into_iter()
            .map(|e| e.unwrap())
            .filter(|e| e.file_type().is_file())
            .map(|e| e.into_path())
            .filter(|p| p.extension() == Some(flac_ext));

        for p in paths_iter {
            // Print progress updates on the number of files discovered.
            // Enumerating the filesystem can take a long time when the OS
            // caches are cold. When the caches are warm it is pretty much
            // instant, but indexing tends to happen with cold caches.
            k += 1;
            if k % 64 == 0 {
                write!(&mut lock, "\r{} files discovered", k);
                lock.flush().unwrap();
            }
            paths.push(p);
        }
        writeln!(&mut lock, "\r{} files discovered", k);

        index = mindec::MemoryMetaIndex::from_paths(paths.iter(), &mut lock);
    };

    let index = index.expect("Failed to build index.");
    println!("Index has {} tracks.", index.len());
    index
}

fn generate_thumbnail(cache_dir: &str, album_id: AlbumId, filename: &str) -> claxon::Result<()> {
    use std::process::{Command, Stdio};
    let opts = claxon::FlacReaderOptions {
        metadata_only: true,
        read_picture: claxon::ReadPicture::CoverAsVec,
        read_vorbis_comment: false,
    };
    let reader = claxon::FlacReader::open_ext(filename, opts)?;
    if let Some(cover) = reader.into_pictures().pop() {
        let mut out_fname: PathBuf = PathBuf::from(cache_dir);
        out_fname.push(format!("{}.jpg", album_id));
        println!("{:?} <- {}", &out_fname, filename);
        let mut convert = Command::new("convert")
            // Read from stdin.
            .arg("-")
            .args(&["-colorspace", "LAB"])
            // Lanczos2 is a bit less sharp than Cosine, but less sharp edges
            // means that the image compresses better, and less artifacts. But
            // still, Lanczos was too blurry in my opinion.
            .args(&["-filter", "Cosine"])
            // Twice the size of the thumb in the webinterface, so they appear
            // pixel-perfect on a high-DPI display, or on a mobile phone.
            .args(&["-distort", "Resize", "140x140!"])
            .args(&["-colorspace", "sRGB"])
            .args(&["-quality", "95"])
            .arg(out_fname)
            .stdin(Stdio::piped())
            .stdout(Stdio::inherit())
            .spawn()
            .expect("Failed to spawn Imagemagick's 'convert'.");
        {
            let mut stdin = convert.stdin.as_mut().expect("Failed to open stdin.");
            stdin.write_all(cover.data());
        }
        // TODO: Use a custom error type, remove all `expect()`s.
        convert.wait().expect("Failed to run Imagemagick's 'convert'.");
    }
    Ok(())
}

fn generate_thumbnails(index: &MemoryMetaIndex, cache_dir: &str) {
    let mut prev_album_id = AlbumId(0);
    for &(_tid, ref track) in index.get_tracks() {
        if track.album_id != prev_album_id {
            let fname = index.get_filename(track.filename);
            generate_thumbnail(cache_dir, track.album_id, fname).unwrap();
            prev_album_id = track.album_id;
        }
    }
}

/// Returns the name of the Chromecast device as present in the TXT record.
fn get_name_from_txt_record(txt: &str) -> Option<String> {
    // The TXT record has the following format:
    //
    // "rs="
    // "nf=1"
    // "bs=<something uppercase hex>"
    // "st=0"
    // "ca=2052"
    // "fn=<chromecast name>"
    // "ic=/setup/icon.png"
    // "md=Chromecast Audio"
    // "ve=05"
    // "rm=<something uppercase hex>"
    // "cd=<something uppercase hex>"
    // "id=<something lowercase hex>"
    //
    // However, this is not how it is passed ... See also
    // https://github.com/tailhook/dns-parser/issues/29
    // For now, deal with it. Upgrade later.
    if let Some(i) = txt.find("fn=") {
        // Cut off the `fn=` prefix.
        let remainder = &txt[i + 3..];
        if let Some(j) = remainder.find("ca=") {
            // Drop until the next key.
            return Some(String::from(&remainder[..j]))
        }
    }
    None
}

fn run_cast() {
    use mdns::{Record, RecordKind};
    use std::net::IpAddr;
    for response in mdns::discover::all("_googlecast._tcp.local").unwrap() {
        let mut response = response.unwrap();
        let mut addr: Option<IpAddr> = None;
        let mut name: Option<String> = None;

        for record in response.records() {
            match record.kind {
                RecordKind::A(addr_v4) => addr = Some(addr_v4.into()),
                RecordKind::AAAA(addr_v6) => addr = Some(addr_v6.into()),
                RecordKind::TXT(ref txt) => name = get_name_from_txt_record(&txt[..]),
                _ => {}
            }
        }
        match (addr, name) {
            (Some(addr), Some(name)) => {
                println!("Found {} at {}.", name, addr);
                break
            }
            (Some(addr), _) => {
                println!("Found nameless cast at {}.", addr);
            }
            _ => continue,
        }
    }
}

fn print_usage() {
    println!("usage: ");
    println!("  mindec serve /path/to/music/library /path/to/cache");
    println!("  mindec cache /path/to/music/library /path/to/cache");
}

fn main() {
    if env::args().len() < 4 {
        print_usage();
        process::exit(1);
    }

    let cmd = env::args().nth(1).unwrap();
    let dir = env::args().nth(2).unwrap();
    let cache_dir = env::args().nth(3).unwrap();

    match &cmd[..] {
        "serve" => {
            let index = make_index(&dir);
            println!("Indexing complete, starting server on port 8233.");
            let service = Rc::new(MetaServer::new(index, &cache_dir));
            let addr = ([0, 0, 0, 0], 8233).into();
            let server = Http::new().bind(&addr, move || Ok(service.clone())).unwrap();
            server.run().unwrap();
        }
        "cache" => {
            let index = make_index(&dir);
            generate_thumbnails(&index, &cache_dir);
        }
        "cast" => {
            run_cast();
        }
        _ => {
            print_usage();
            process::exit(1);
        }
    }
}
