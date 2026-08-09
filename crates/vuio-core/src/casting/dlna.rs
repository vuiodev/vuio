use super::{
    CastProvider, PlaybackAction, PlaybackItem, PlaybackState, PlaybackStatus,
    RendererCapabilities, RendererDevice, RendererEndpoint, RendererProtocol,
};
use async_trait::async_trait;
use std::time::Duration;

pub struct DlnaProvider;

#[async_trait]
impl CastProvider for DlnaProvider {
    fn protocol(&self) -> RendererProtocol {
        RendererProtocol::Dlna
    }

    async fn discover(&self, _timeout: Duration) -> anyhow::Result<Vec<RendererDevice>> {
        Ok(crate::tv_control::discover_tvs()
            .await?
            .into_iter()
            .map(|tv| RendererDevice {
                id: tv.id,
                friendly_name: tv.friendly_name,
                control_url: tv.control_url.clone(),
                location_url: tv.location_url.clone(),
                model_name: tv.model_name,
                protocol: RendererProtocol::Dlna,
                pairing: super::PairingStatus::NotRequired,
                capabilities: RendererCapabilities {
                    video: true,
                    audio: true,
                    image: true,
                    playlists: true,
                    controls: vec![
                        PlaybackAction::Play,
                        PlaybackAction::Pause,
                        PlaybackAction::Stop,
                    ],
                },
                endpoint: RendererEndpoint::Dlna {
                    control_url: tv.control_url,
                    location_url: tv.location_url,
                },
            })
            .collect())
    }

    fn validate(&self, _item: &PlaybackItem) -> Result<(), String> {
        Ok(())
    }

    async fn play(&self, device: &RendererDevice, item: &PlaybackItem) -> anyhow::Result<()> {
        crate::tv_control::cast_media(
            dlna_control_url(device)?,
            &item.url,
            &item.title,
            &item.mime_type,
        )
        .await
    }

    async fn control(&self, device: &RendererDevice, action: PlaybackAction) -> anyhow::Result<()> {
        let action = match action {
            PlaybackAction::Play => "Play",
            PlaybackAction::Pause => "Pause",
            PlaybackAction::Stop => "Stop",
        };
        crate::tv_control::control_playback(dlna_control_url(device)?, action).await
    }

    async fn status(&self, device: &RendererDevice) -> anyhow::Result<PlaybackStatus> {
        let url = dlna_control_url(device)?;
        let (state, current_url) = tokio::join!(
            crate::tv_control::get_transport_state(url),
            crate::tv_control::get_position_info(url)
        );
        let state = match state?.as_str() {
            "PLAYING" => PlaybackState::Playing,
            "PAUSED_PLAYBACK" => PlaybackState::Paused,
            "STOPPED" => PlaybackState::Stopped,
            _ => PlaybackState::Unknown,
        };
        Ok(PlaybackStatus {
            state,
            current_url: current_url.ok().filter(|value| !value.is_empty()),
        })
    }

    async fn queue_next(
        &self,
        device: &RendererDevice,
        item: &PlaybackItem,
    ) -> anyhow::Result<bool> {
        crate::tv_control::set_next_media(
            dlna_control_url(device)?,
            &item.url,
            &item.title,
            &item.mime_type,
        )
        .await?;
        Ok(true)
    }
}

fn dlna_control_url(device: &RendererDevice) -> anyhow::Result<&str> {
    match &device.endpoint {
        RendererEndpoint::Dlna { control_url, .. } => Ok(control_url),
        _ => anyhow::bail!("invalid DLNA renderer endpoint"),
    }
}
