use super::*;

pub async fn connection_manager_scpd() -> impl IntoResponse {
    let xml = crate::web::xml::generate_connection_manager_scpd();
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/xml; charset=utf-8")],
        xml,
    )
}

pub async fn media_receiver_registrar_scpd() -> impl IntoResponse {
    let xml = crate::web::xml::generate_registrar_scpd();
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/xml; charset=utf-8")],
        xml,
    )
}

pub async fn connection_manager_control<D: DatabaseManager>(
    State(_state): State<AppState<D>>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let action = match soap_action(&headers, &body) {
        Ok(action) => action,
        Err(response) => return *response,
    };
    if action == "GetProtocolInfo" {
        let content = r#"<Source>http-get:*:video/x-msvideo:*,http-get:*:video/mp4:*,http-get:*:video/x-matroska:*,http-get:*:video/x-mkv:*,http-get:*:video/mpeg:*,http-get:*:video/divx:*,http-get:*:audio/mpeg:*,http-get:*:audio/x-flac:*,http-get:*:audio/wav:*,http-get:*:audio/mp4:*,http-get:*:image/jpeg:*,http-get:*:image/png:*,http-get:*:image/gif:*</Source><Sink></Sink>"#;
        build_soap_response(
            "GetProtocolInfo",
            "urn:schemas-upnp-org:service:ConnectionManager:1",
            content,
        )
    } else if action == "GetCurrentConnectionIDs" {
        let content = "<ConnectionIDs>0</ConnectionIDs>";
        build_soap_response(
            "GetCurrentConnectionIDs",
            "urn:schemas-upnp-org:service:ConnectionManager:1",
            content,
        )
    } else if action == "GetCurrentConnectionInfo" {
        let content = r#"<RcsID>-1</RcsID><AVTransportID>-1</AVTransportID><ProtocolInfo></ProtocolInfo><PeerConnectionManager></PeerConnectionManager><PeerConnectionID>-1</PeerConnectionID><Direction>Output</Direction><Status>Unknown</Status>"#;
        build_soap_response(
            "GetCurrentConnectionInfo",
            "urn:schemas-upnp-org:service:ConnectionManager:1",
            content,
        )
    } else {
        (
            StatusCode::NOT_IMPLEMENTED,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "Not implemented".to_string(),
        )
            .into_response()
    }
}

pub async fn media_receiver_registrar_control<D: DatabaseManager>(
    State(_state): State<AppState<D>>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let action = match soap_action(&headers, &body) {
        Ok(action) => action,
        Err(response) => return *response,
    };
    if action == "IsAuthorized" {
        let content = "<Result>1</Result>";
        build_soap_response(
            "IsAuthorized",
            "urn:microsoft.com:service:X_MS_MediaReceiverRegistrar:1",
            content,
        )
    } else if action == "RegisterDevice" {
        let content = "<RegistrationRespMsg></RegistrationRespMsg>";
        build_soap_response(
            "RegisterDevice",
            "urn:microsoft.com:service:X_MS_MediaReceiverRegistrar:1",
            content,
        )
    } else {
        (
            StatusCode::NOT_IMPLEMENTED,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "Not implemented".to_string(),
        )
            .into_response()
    }
}
