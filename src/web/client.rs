use axum::http::HeaderMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DlnaClientProfile {
    Xbox,
    PlayStation,
    SamsungTv,
    SonyBdp,
    SonyBravia,
    LgTv,
    Standard,
}

pub fn detect_client(headers: &HeaderMap) -> DlnaClientProfile {
    // 1. Check X-AV-Client-Info header first (commonly used by Sony)
    if let Some(av_info) = headers.get("x-av-client-info").and_then(|h| h.to_str().ok()) {
        let av_info_lower = av_info.to_lowercase();
        if av_info_lower.contains("playstation 3") || av_info_lower.contains("playstation") {
            return DlnaClientProfile::PlayStation;
        } else if av_info_lower.contains("blu-ray disc player") || 
                  av_info_lower.contains("blu-ray home theatre system") || 
                  av_info_lower.contains("media player") {
            return DlnaClientProfile::SonyBdp;
        } else if av_info_lower.contains("bravia") || av_info_lower.contains("internet tv") {
            return DlnaClientProfile::SonyBravia;
        }
    }

    // 2. Check User-Agent header
    if let Some(user_agent) = headers.get(axum::http::header::USER_AGENT).and_then(|h| h.to_str().ok()) {
        let ua_lower = user_agent.to_lowercase();
        if ua_lower.contains("xbox") {
            return DlnaClientProfile::Xbox;
        } else if ua_lower.contains("playstation") || ua_lower.contains("ps3") || ua_lower.contains("ps4") {
            return DlnaClientProfile::PlayStation;
        } else if ua_lower.contains("sec_hhp_") || ua_lower.contains("samsungwiselinkpro") || ua_lower.contains("samsung") {
            return DlnaClientProfile::SamsungTv;
        } else if ua_lower.contains("lge_dlna_sdk") || ua_lower.contains("lg player") || ua_lower.contains("lg-tv") {
            return DlnaClientProfile::LgTv;
        } else if ua_lower.contains("sony bdp") || ua_lower.contains("blu-ray") {
            return DlnaClientProfile::SonyBdp;
        } else if ua_lower.contains("bravia") {
            return DlnaClientProfile::SonyBravia;
        } else if ua_lower.contains("dlnadoc/1.50") {
            return DlnaClientProfile::Standard;
        }
    }

    DlnaClientProfile::Standard
}

tokio::task_local! {
    pub static CURRENT_CLIENT: DlnaClientProfile;
}
