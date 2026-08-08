use super::*;

pub async fn sse_handler<D: DatabaseManager + 'static>(
    State(state): State<AppState<D>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
) -> axum::response::Response {
    let client_id = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let (tx, rx) = mpsc::channel::<String>(64);
    let disconnect_sender = tx.clone();
    let expires_at = Instant::now() + MCP_CLIENT_TTL;

    // Register this client
    {
        let mut clients = state.mcp_clients.lock().await;
        let now = Instant::now();
        clients.retain(|_, client| client.expires_at > now);
        let peer_clients = clients
            .values()
            .filter(|client| client.peer == peer.ip())
            .count();
        if clients.len() >= MCP_MAX_CLIENTS || peer_clients >= MCP_MAX_CLIENTS_PER_PEER {
            return StatusCode::TOO_MANY_REQUESTS.into_response();
        }
        clients.insert(
            client_id.clone(),
            McpClient {
                sender: tx,
                peer: peer.ip(),
                expires_at,
            },
        );
    }

    info!("MCP client connected: {}", client_id);

    // Build the SSE stream
    let client_id_for_cleanup = client_id.clone();
    let state_for_cleanup = state.clone();

    let endpoint_url = format!("/mcp/message?client_id={}", client_id);

    let initial_event = Event::default().event("endpoint").data(endpoint_url);

    let rx_stream =
        ReceiverStream::new(rx).map(|msg| Ok(Event::default().event("message").data(msg)));

    let stream = futures_util::stream::once(async move { Ok::<_, Infallible>(initial_event) })
        .chain(rx_stream);

    // Sender::closed resolves as soon as Axum drops the SSE receiver, so stale
    // clients are removed immediately rather than by a coarse timeout.
    let cleanup_cancellation = state.cancellation.clone();
    state.background_tasks.spawn(async move {
        tokio::select! {
            _ = disconnect_sender.closed() => {}
            _ = cleanup_cancellation.cancelled() => {}
            _ = tokio::time::sleep_until(tokio::time::Instant::from_std(expires_at)) => {}
        }
        let mut clients = state_for_cleanup.mcp_clients.lock().await;
        let is_same_connection = clients
            .get(&client_id_for_cleanup)
            .is_some_and(|client| client.sender.same_channel(&disconnect_sender));
        if is_same_connection {
            clients.remove(&client_id_for_cleanup);
            info!("MCP client disconnected: {}", client_id_for_cleanup);
        }
    });

    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

// ──────────────────────────────────────────
// Message Handler — POST /mcp/message
// ──────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct MessageQuery {
    pub client_id: String,
}

pub async fn message_handler<D: DatabaseManager + 'static>(
    State(state): State<AppState<D>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Query(query): Query<MessageQuery>,
    Json(request): Json<JsonRpcRequest>,
) -> impl IntoResponse {
    let client_id = &query.client_id;

    // Validate the live, peer-bound capability before dispatching any method.
    let sender = {
        let mut clients = state.mcp_clients.lock().await;
        let now = Instant::now();
        clients.retain(|_, client| client.expires_at > now);
        clients
            .get(client_id)
            .filter(|client| client.peer == peer.ip())
            .map(|client| client.sender.clone())
    };
    let Some(sender) = sender else {
        return StatusCode::UNAUTHORIZED.into_response();
    };

    debug!("MCP request from {}: method={}", client_id, request.method);

    // Handle the method
    let response = handle_method(&state, &request).await;

    // Send the response back through the SSE channel
    let response_json = match serde_json::to_string(&response) {
        Ok(response) if response.len() <= MCP_MAX_RESPONSE_BYTES => response,
        Ok(_) => {
            return (
                StatusCode::PAYLOAD_TOO_LARGE,
                axum::Json(JsonRpcResponse::error(
                    request.id.clone(),
                    -32001,
                    "Response exceeds server limit".to_owned(),
                )),
            )
                .into_response();
        }
        Err(error) => {
            warn!("Failed to serialize MCP response: {error}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    if let Err(e) = sender.send(response_json).await {
        warn!("Failed to send MCP response to client {}: {}", client_id, e);
        let mut clients = state.mcp_clients.lock().await;
        if clients
            .get(client_id)
            .is_some_and(|client| client.sender.same_channel(&sender))
        {
            clients.remove(client_id);
        }
        return StatusCode::GONE.into_response();
    }
    (StatusCode::ACCEPTED, "").into_response()
}
