#[cfg(feature = "dashboard")]
pub mod admin;
pub mod auth;
#[cfg(feature = "casting")]
pub mod casting;
pub mod client;
pub mod diagnostics;
pub mod eventing;
mod format;
#[cfg(feature = "mcp")]
pub mod mcp;
#[cfg(all(feature = "dashboard", feature = "mediainfo"))]
pub mod mediainfo;
#[cfg(feature = "casting")]
pub mod remux_streaming;
pub mod radio;
pub mod soap;
pub mod streaming;
pub mod subtitles;
#[cfg(feature = "transcode")]
pub mod transcode_streaming;
#[cfg(all(feature = "transcode-aac", feature = "casting"))]
pub mod video_streaming;
#[cfg(feature = "dashboard")]
pub mod ui;
pub mod xml;

use crate::{database::DatabaseManager, state::AppState};

/// Whether this item is one a renderer may be unable to play unaided.
///
/// True only when the codec is AC-3, E-AC-3 or DTS *and* this build can decode
/// it — advertising a resource we cannot produce would turn a silent film into
/// a broken one. The recorded codec is consulted first because it is what a
/// container's audio track will be identified by; the MIME type and filename
/// cover an elementary stream, including one indexed before those MIME types
/// existed.
#[cfg_attr(not(feature = "transcode"), allow(unused_variables))]
pub(crate) fn item_needs_transcode(codec: Option<&str>, mime: &str, filename: &str) -> bool {
    #[cfg(not(feature = "transcode"))]
    {
        false
    }
    #[cfg(feature = "transcode")]
    {
        use crate::media::transcode::TranscodeCodec;
        codec
            .and_then(TranscodeCodec::from_stored_codec)
            .or_else(|| transcode_streaming::codec_for(mime, filename))
            .is_some_and(TranscodeCodec::is_decodable)
    }
}

/// Whether a film's picture can be copied into the remuxed alternative.
///
/// Needing an alternative and being able to produce one are different
/// questions, asked of different tracks. The audio decides the first; the video
/// decides the second, because the alternative copies the picture through
/// rather than re-encoding it and can only do that for the codecs the fMP4
/// writer knows how to describe. A film with a VP9 or MPEG-2 picture and an AC-3
/// soundtrack therefore gets no second resource — advertising one and answering
/// 404 would be worse than the silence it was meant to fix.
///
/// A record written before the scanner recorded video codecs carries `None`.
/// Those are treated as remuxable: the next scan fills the column in, and until
/// it does, the far more common case is the one that works.
pub(crate) fn item_can_remux_video(video_codec: Option<&str>) -> bool {
    match video_codec {
        None => true,
        Some(codec) => matches!(
            codec.trim().to_ascii_lowercase().as_str(),
            "h264" | "avc" | "avc1" | "hevc" | "h265" | "hvc1"
        ),
    }
}

/// How this server should advertise a decoded alternative, if at all.
///
/// One place decides, so the two DIDL writers cannot drift apart on it, and the
/// feature gate lives here rather than in the XML.
pub(crate) fn transcode_advert<D: DatabaseManager>(
    state: &AppState<D>,
) -> Option<xml::TranscodeAdvert> {
    #[cfg(not(feature = "transcode"))]
    {
        let _ = state;
        None
    }
    #[cfg(feature = "transcode")]
    {
        use crate::config::{TranscodeAudioFormat, TranscodeMode};
        let config = state.current_config();
        if !config.transcode.enabled || config.transcode.mode == TranscodeMode::Disabled {
            return None;
        }
        Some(xml::TranscodeAdvert {
            // The MIME differs from the original's, which is what lets a
            // renderer that matches against its own sink protocolInfo pick the
            // one it can actually decode.
            audio: match config.transcode.audio_format {
                TranscodeAudioFormat::Lpcm => xml::AdvertResource {
                    mime: "audio/vnd.wave",
                    path: "transcode/audio.wav",
                    // Constant-bitrate PCM: a byte offset divides straight back
                    // into a sample, so this is a real seek.
                    op: "11",
                },
                TranscodeAudioFormat::Aac => xml::AdvertResource {
                    mime: "audio/aac",
                    path: "transcode/audio.aac",
                    // A lossy re-encode has no length until it exists, so there
                    // is nothing to seek within.
                    op: "00",
                },
            },
            // A film is offered the film, not its soundtrack: the same picture,
            // with an audio track the renderer can actually decode. Time seek
            // only — see `web::video_streaming` for why byte seek is not on
            // offer and why time seek is enough.
            #[cfg(all(feature = "transcode-aac", feature = "casting"))]
            video: Some(xml::AdvertResource {
                mime: "video/mp4",
                path: "transcode/video.mp4",
                op: "10",
            }),
            // With no remuxer or no encoder there is nothing to offer a film.
            // Offering it `audio.wav` instead would replace a silent film with
            // no film at all.
            #[cfg(not(all(feature = "transcode-aac", feature = "casting")))]
            video: None,
            first: config.transcode.mode == TranscodeMode::Forced,
        })
    }
}
use axum::{
    extract::DefaultBodyLimit,
    middleware,
    routing::{get, post},
    Router,
};

const SOAP_BODY_LIMIT: usize = 1024 * 1024;
const JSON_BODY_LIMIT: usize = 256 * 1024;

/// Which listener a router is being built for.
///
/// The server answers on two ports, and they differ in exactly one respect:
/// what lives at `/`. Everything else — DLNA, streaming, the management API —
/// is the same routes over the same `AppState`, so the second listener is a
/// second front end rather than a second server. In particular the browser app
/// reaches the database through the same handlers as the dashboard, in
/// process, with no proxy hop between them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Surface {
    /// The main listener: DLNA/UPnP, streaming, and the built-in dashboard.
    Primary,
    /// The secondary listener: the same API, with the `vuio-web` app at `/`.
    #[cfg(feature = "web-ui")]
    WebUi,
}

impl Surface {
    /// Whether the built-in dashboard's own page and assets belong on this
    /// listener. They do not on the secondary one, where the Svelte app owns
    /// `/` and `/assets` and brings its own copies of the player libraries.
    #[cfg(feature = "dashboard")]
    fn serves_builtin_dashboard(self) -> bool {
        matches!(self, Self::Primary)
    }
}

/// With neither front end compiled in there is nothing at `/` to choose
/// between, so the surface stops being consulted. The parameter stays rather
/// than becoming another `#[cfg]` on every call site.
#[cfg_attr(not(feature = "dashboard"), allow(unused_variables))]
pub fn create_router<D: DatabaseManager + 'static>(
    state: AppState<D>,
    surface: Surface,
) -> Router {
    let soap_routes = Router::new()
        .route(
            "/control/ContentDirectory",
            get(soap::content_directory_control::<D>).post(soap::content_directory_control::<D>),
        )
        .route(
            "/control/ConnectionManager",
            get(soap::connection_manager_control::<D>).post(soap::connection_manager_control::<D>),
        )
        .route(
            "/control/X_MS_MediaReceiverRegistrar",
            get(soap::media_receiver_registrar_control::<D>)
                .post(soap::media_receiver_registrar_control::<D>),
        )
        .layer(DefaultBodyLimit::max(SOAP_BODY_LIMIT));

    // Routes are added conditionally rather than declared in one chain: a
    // feature that is off must take its endpoints with it, so a caller gets a
    // 404 instead of a handler that cannot work.
    #[allow(unused_mut)]
    let mut json_routes = Router::new();
    #[cfg(feature = "casting")]
    {
        json_routes = json_routes
            .route("/api/cast", post(casting::api_cast::<D>))
            .route("/api/cast/control", post(casting::api_cast_control::<D>))
            .route("/api/cast/playlist", post(casting::api_cast_playlist::<D>))
            .route(
                "/api/renderers/pair/start",
                post(casting::api_pairing_start::<D>),
            )
            .route(
                "/api/renderers/pair/finish",
                post(casting::api_pairing_finish::<D>),
            )
            .route(
                "/api/renderers/pair/forget",
                post(casting::api_pairing_forget::<D>),
            );
    }
    // The MCP endpoint lives on the primary listener only. It is one endpoint
    // over one AppState, and publishing it on both ports would give an agent
    // two addresses for the same server with nothing to choose between them.
    #[cfg(feature = "mcp")]
    if surface == Surface::Primary && state.current_config().mcp.enabled {
        json_routes = json_routes.route(
            "/mcp",
            post(mcp::mcp_handler::<D>)
                // GET and DELETE belonged to the session-based revisions of the
                // transport. Answering 405 is how an older client finds out.
                .get(mcp::method_not_allowed)
                .delete(mcp::method_not_allowed),
        );
    }
    #[cfg(feature = "dashboard")]
    {
        json_routes = json_routes
            .route("/api/admin/config", post(admin::put_config::<D>))
            .route("/api/admin/restart", post(admin::restart::<D>))
            .route("/api/radio/admin/stations", post(radio::create_station::<D>))
            .route(
                "/api/radio/admin/stations/{id}",
                post(radio::update_station::<D>),
            )
            .route(
                "/api/radio/admin/stations/{id}/start",
                post(radio::start_station::<D>),
            )
            .route(
                "/api/radio/admin/stations/{id}/stop",
                post(radio::stop_station::<D>),
            )
            .route(
                "/api/radio/admin/stations/{id}/skip",
                post(radio::skip_track::<D>),
            )
            .route(
                "/api/radio/admin/stations/{id}/delete",
                post(radio::delete_station::<D>),
            );
    }
    #[cfg(all(feature = "dashboard", feature = "mediainfo"))]
    {
        json_routes = json_routes
            .route(
                "/api/admin/mediainfo/credentials",
                post(mediainfo::put_credential::<D>),
            )
            .route("/api/admin/mediainfo/run", post(mediainfo::run::<D>))
            .route("/api/admin/mediainfo/cancel", post(mediainfo::cancel::<D>));
    }
    let json_routes = json_routes.layer(DefaultBodyLimit::max(JSON_BODY_LIMIT));

    #[allow(unused_mut)]
    let mut management_routes = Router::new()
        .route("/metrics", get(diagnostics::get_prometheus_metrics::<D>))
        .route("/metrics/json", get(diagnostics::get_web_metrics::<D>))
        .route("/logs", get(diagnostics::get_logs_handler::<D>))
        .route("/logout", post(auth::logout::<D>))
        .route("/api/radio/admin/stations", get(radio::list_stations::<D>))
        .route("/api/radio/peers", get(radio::list_peers::<D>));
    #[cfg(feature = "dashboard")]
    {
        management_routes = management_routes
            .route("/api/server-info", get(ui::server_info_handler::<D>))
            .route("/api/media", get(ui::media_page_handler::<D>))
            .route("/api/browse", get(ui::browse_handler::<D>))
            .route("/api/admin/config", get(admin::get_config::<D>));
        if surface.serves_builtin_dashboard() {
            management_routes = management_routes.route("/", get(ui::root_handler));
        }
    }
    #[cfg(all(feature = "dashboard", feature = "mediainfo"))]
    {
        management_routes =
            management_routes.route("/api/admin/mediainfo", get(mediainfo::get_status::<D>));
    }
    #[cfg(feature = "casting")]
    {
        management_routes =
            management_routes.route("/api/renderers", get(casting::api_list_renderers::<D>));
    }
    // The dashboard's own stylesheets and scripts are part of the management surface,
    // so they sit behind the same middleware as the page that loads them. That is only
    // safe because `require_management` excludes /assets from its login-page redirect:
    // a 200 login page returned for a <script> tag would be parsed as JavaScript.
    #[cfg(feature = "dashboard")]
    if surface.serves_builtin_dashboard() {
        management_routes = management_routes.route("/assets/{file}", get(ui::asset_handler));
    }
    let management_routes = management_routes
        .merge(json_routes)
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_management::<D>,
        ));

    let router = Router::new()
        .route("/login", get(auth::login_page::<D>).post(auth::login::<D>))
        .route("/description.xml", get(soap::description_handler::<D>))
        .route("/ContentDirectory.xml", get(soap::content_directory_scpd))
        .route(
            "/event/ContentDirectory",
            axum::routing::any(eventing::content_directory_subscribe::<D>),
        )
        .route("/ConnectionManager.xml", get(soap::connection_manager_scpd))
        .route(
            "/X_MS_MediaReceiverRegistrar.xml",
            get(soap::media_receiver_registrar_scpd),
        )
        .route(
            "/media/{id}",
            get(streaming::serve_media::<D>).head(streaming::serve_media::<D>),
        )
        // A station's audio and the list of what is on the air are public, for
        // the same reason `/media/{id}` is: the things that play a stream — a
        // hi-fi, VLC, another VuIO server building its local-stations list —
        // have nowhere to put a login. Running the stations stays behind one.
        .route("/api/radio/stations", get(radio::list_public_stations::<D>))
        .route(
            "/api/radio/stations/{id}/stream",
            get(radio::serve_stream::<D>),
        )
        .route(
            "/api/radio/stations/{id}/stream.{extension}",
            get(radio::serve_stream_with_extension::<D>),
        );

    // Public for the same reason `/media/{id}` is: a TV playing the decoded
    // version of a film has nowhere to put a login either.
    #[cfg(feature = "transcode")]
    let router = router.route(
        "/media/{id}/transcode/audio.wav",
        get(transcode_streaming::serve_transcoded_wav::<D>)
            .head(transcode_streaming::serve_transcoded_wav::<D>),
    );
    #[cfg(feature = "transcode-aac")]
    let router = router.route(
        "/media/{id}/transcode/audio.aac",
        get(transcode_streaming::serve_transcoded_aac::<D>)
            .head(transcode_streaming::serve_transcoded_aac::<D>),
    );
    // The film itself, remuxed with its audio decoded. Needs the demuxer as
    // well as the encoder, which is why it rides on `casting` too.
    #[cfg(all(feature = "transcode-aac", feature = "casting"))]
    let router = router.route(
        "/media/{id}/transcode/video.mp4",
        get(video_streaming::serve_transcoded_video::<D>)
            .head(video_streaming::serve_transcoded_video::<D>),
    );

    #[cfg(feature = "casting")]
    let router = router
        .route(
            "/media/{id}/hls/master.m3u8",
            get(remux_streaming::serve_hls_master::<D>),
        )
        .route(
            "/media/{id}/hls/video/index.m3u8",
            get(remux_streaming::serve_hls_video_playlist::<D>),
        )
        .route(
            "/media/{id}/hls/audio/{idx}/index.m3u8",
            get(remux_streaming::serve_hls_audio_playlist::<D>),
        )
        .route(
            "/media/{id}/hls/video/init.mp4",
            get(remux_streaming::serve_hls_video_init_segment::<D>),
        )
        .route(
            "/media/{id}/hls/video/segment/{seq}",
            get(remux_streaming::serve_hls_video_segment::<D>),
        )
        .route(
            "/media/{id}/hls/audio/{idx}/init.mp4",
            get(remux_streaming::serve_hls_audio_init_segment::<D>),
        )
        .route(
            "/media/{id}/hls/audio/{idx}/segment/{seq}",
            get(remux_streaming::serve_hls_audio_segment::<D>),
        );

    let router = router
        .route("/media/{id}/cover", get(streaming::serve_cover::<D>))
        .route("/media/{id}/subtitle", get(streaming::serve_subtitle::<D>))
        .route(
            "/media/{id}/subtitle.vtt",
            get(streaming::serve_subtitle_vtt::<D>),
        )
        .route("/healthz", get(diagnostics::healthz_handler))
        .route("/readyz", get(diagnostics::readyz_handler::<D>))
        .merge(soap_routes)
        .merge(management_routes);

    // The browser app is management surface like the dashboard, so it sits
    // behind the same middleware. `layer` rather than `route_layer` because the
    // app arrives as a fallback: `route_layer` runs only for matched routes,
    // which would leave every client-side route of the app unauthenticated.
    #[cfg(feature = "web-ui")]
    let router = if surface == Surface::WebUi {
        router.merge(
            vuio_web::routes::<AppState<D>>().layer(middleware::from_fn_with_state(
                state.clone(),
                auth::require_management::<D>,
            )),
        )
    } else {
        router
    };

    router.with_state(state)
}

