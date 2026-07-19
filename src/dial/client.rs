//! DIAL protocol client for managing apps on smart TVs.
//!
//! DIAL allows launching and stopping apps (YouTube, Netflix, etc.)
//! on smart TVs via a simple REST API discovered through SSDP.

use anyhow::{Context, Result};
use std::time::Duration;
use tracing::debug;

/// DIAL application controller for a specific device.
pub struct DialClient {
    /// The DIAL Application-URL base for this device.
    application_url: String,
    client: reqwest::Client,
}

/// Status of a DIAL application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppStatus {
    Running,
    Stopped,
    Installable,
    NotAvailable,
}

/// Information about a DIAL app on a device.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AppInfo {
    pub name: String,
    pub status: AppStatus,
    /// URL to use for stopping a running instance.
    pub instance_url: Option<String>,
}

impl DialClient {
    /// Create a new DIAL client for a device with the given Application-URL.
    pub fn new(application_url: &str) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("failed to build HTTP client");

        Self {
            application_url: application_url.trim_end_matches('/').to_string(),
            client,
        }
    }

    /// Query the status of an app on this device.
    pub async fn query_app(&self, app_name: &str) -> Result<AppInfo> {
        let url = format!("{}/{}", self.application_url, app_name);

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .context("failed to query DIAL app")?;

        match resp.status().as_u16() {
            200 => {
                let body = resp.text().await.unwrap_or_default();
                let state = extract_xml_element(&body, "state").unwrap_or_default();
                let instance_url = extract_instance_url(&body, &self.application_url, app_name);

                let status = match state.as_str() {
                    "running" => AppStatus::Running,
                    "stopped" => AppStatus::Stopped,
                    "installable" => AppStatus::Installable,
                    _ => AppStatus::Stopped,
                };

                Ok(AppInfo {
                    name: app_name.to_string(),
                    status,
                    instance_url,
                })
            }
            404 => Ok(AppInfo {
                name: app_name.to_string(),
                status: AppStatus::NotAvailable,
                instance_url: None,
            }),
            status => {
                anyhow::bail!("DIAL app query returned HTTP {}", status);
            }
        }
    }

    /// Launch an app on this device, optionally with a body payload.
    pub async fn launch_app(&self, app_name: &str, launch_data: Option<&str>) -> Result<()> {
        let url = format!("{}/{}", self.application_url, app_name);

        let mut req = self.client.post(&url);
        if let Some(data) = launch_data {
            req = req
                .header("Content-Type", "text/plain; charset=utf-8")
                .body(data.to_string());
        } else {
            // DIAL spec requires Content-Length: 0 for empty POST
            req = req.header("Content-Length", "0");
        }

        let resp = req.send().await.context("failed to launch DIAL app")?;

        match resp.status().as_u16() {
            200 | 201 => {
                debug!("DIAL app {} launched successfully", app_name);
                Ok(())
            }
            status => {
                anyhow::bail!("DIAL app launch failed with HTTP {}", status);
            }
        }
    }

    /// Stop a running app instance.
    pub async fn stop_app(&self, app_name: &str) -> Result<()> {
        // First query the app to get its running instance URL
        let info = self.query_app(app_name).await?;

        let stop_url = match info.instance_url {
            Some(url) => url,
            None => {
                // Fallback: try the standard /run suffix
                format!("{}/{}/run", self.application_url, app_name)
            }
        };

        let resp = self
            .client
            .delete(&stop_url)
            .send()
            .await
            .context("failed to stop DIAL app")?;

        if resp.status().is_success() || resp.status().as_u16() == 404 {
            debug!("DIAL app {} stopped", app_name);
            Ok(())
        } else {
            anyhow::bail!("DIAL app stop failed with HTTP {}", resp.status());
        }
    }

    /// Query available apps on this device.
    /// Returns the status of commonly known DIAL apps.
    pub async fn query_common_apps(&self) -> Vec<AppInfo> {
        let common_apps = ["YouTube", "Netflix", "AmazonInstantVideo", "Hulu"];
        let mut results = Vec::new();

        for app in &common_apps {
            match self.query_app(app).await {
                Ok(info) if info.status != AppStatus::NotAvailable => {
                    results.push(info);
                }
                Ok(_) => {
                    // App not available, skip
                }
                Err(e) => {
                    debug!("Failed to query DIAL app {}: {}", app, e);
                }
            }
        }

        results
    }
}

/// Simple XML element text extractor.
fn extract_xml_element(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    Some(xml[start..end].trim().to_string())
}

/// Extract the instance URL from a DIAL app response.
/// The `<link>` element with rel="run" contains the running instance URL.
fn extract_instance_url(body: &str, app_url: &str, app_name: &str) -> Option<String> {
    // Look for href in a <link> element
    if let Some(link_start) = body.find("<link") {
        let link_section = &body[link_start..];
        if let Some(href_start) = link_section.find("href=\"") {
            let href = &link_section[href_start + 6..];
            if let Some(href_end) = href.find('"') {
                let href_value = &href[..href_end];
                // If it's a relative URL, make it absolute
                if href_value.starts_with("http") {
                    return Some(href_value.to_string());
                } else {
                    return Some(format!(
                        "{}/{}/{}",
                        app_url,
                        app_name,
                        href_value.trim_start_matches('/')
                    ));
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_state_from_response() {
        let xml = r#"<?xml version="1.0"?>
<service xmlns="urn:dial-multiscreen-org:schemas:dial">
  <name>YouTube</name>
  <state>running</state>
  <link rel="run" href="run"/>
</service>"#;
        assert_eq!(
            extract_xml_element(xml, "state"),
            Some("running".to_string())
        );
    }

    #[test]
    fn extract_instance_url_relative() {
        let body = r#"<service><state>running</state><link rel="run" href="run"/></service>"#;
        let result = extract_instance_url(body, "http://192.168.1.10/apps", "YouTube");
        assert_eq!(
            result,
            Some("http://192.168.1.10/apps/YouTube/run".to_string())
        );
    }

    #[test]
    fn extract_instance_url_absolute() {
        let body =
            r#"<service><link rel="run" href="http://192.168.1.10/apps/YouTube/run"/></service>"#;
        let result = extract_instance_url(body, "http://192.168.1.10/apps", "YouTube");
        assert_eq!(
            result,
            Some("http://192.168.1.10/apps/YouTube/run".to_string())
        );
    }

    #[test]
    fn dial_client_trims_trailing_slash() {
        let client = DialClient::new("http://192.168.1.10/apps/");
        assert_eq!(client.application_url, "http://192.168.1.10/apps");
    }
}
