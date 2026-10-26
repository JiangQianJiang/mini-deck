use qrcode::{Color, QrCode};
use std::net::{IpAddr, SocketAddr};

pub fn urls(address: SocketAddr) -> Result<Vec<String>, String> {
    let mut ips = if address.ip().is_unspecified() {
        if_addrs::get_if_addrs()
            .map_err(|e| format!("网卡枚举失败：{e}"))?
            .into_iter()
            .map(|interface| interface.ip())
            .filter(|ip| {
                !ip.is_loopback() && !ip.is_unspecified() && ip.is_ipv4() == address.is_ipv4()
            })
            .filter(|ip| match ip {
                IpAddr::V4(ip) => !ip.is_link_local(),
                IpAddr::V6(ip) => !ip.is_unicast_link_local(),
            })
            .collect::<Vec<_>>()
    } else {
        vec![address.ip()]
    };
    ips.sort();
    ips.dedup();
    Ok(ips
        .into_iter()
        .map(|ip| format!("http://{}", SocketAddr::new(ip, address.port())))
        .collect())
}
pub fn qr_svg(url: &str) -> Result<String, String> {
    let code = QrCode::new(url.as_bytes()).map_err(|e| e.to_string())?;
    let width = code.width();
    let side = width + 8;
    let mut svg = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {side} {side}\" role=\"img\" aria-label=\"扫码连接\" shape-rendering=\"crispEdges\"><rect width=\"{side}\" height=\"{side}\" fill=\"white\"/><path fill=\"black\" d=\"");
    for y in 0..width {
        for x in 0..width {
            if code[(x, y)] == Color::Dark {
                svg.push_str(&format!("M{} {}h1v1h-1z", x + 4, y + 4));
            }
        }
    }
    svg.push_str("\"/></svg>");
    Ok(svg)
}
pub fn page(urls: &[String]) -> Result<String, String> {
    let mut html = String::from("<!doctype html><html lang=\"zh-CN\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Mini Deck 手机连接</title><link rel=\"stylesheet\" href=\"/style.css\"></head><body><main><h1>手机扫码连接</h1><p>手机和电脑连接同一私人局域网，扫码后输入电脑控制台的配对码。二维码仅包含访问地址。</p>");
    for url in urls {
        html.push_str(&format!(
            "<section class=\"connection\">{}<p><a href=\"{url}\">{url}</a></p></section>",
            qr_svg(url)?
        ));
    }
    if urls.is_empty() {
        html.push_str("<p>没有找到匹配的局域网地址，请检查网卡或显式指定 --listen IP:8765。</p>");
    }
    html.push_str("<p>多网卡时选择手机可访问的地址；VPN、虚拟网卡和防火墙可能影响连接。回环地址仅供电脑本机使用。</p><p><a href=\"/\">打开控制面板</a></p></main></body></html>");
    Ok(html)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn urls_and_qr_have_no_credentials() {
        assert_eq!(
            urls("192.168.1.3:8765".parse().unwrap()).unwrap(),
            ["http://192.168.1.3:8765"]
        );
        assert_eq!(
            urls("[::1]:8765".parse().unwrap()).unwrap(),
            ["http://[::1]:8765"]
        );
        let html = page(&urls("127.0.0.1:8765".parse().unwrap()).unwrap()).unwrap();
        assert!(html.contains("<svg"));
        assert!(!html.contains("token="));
        assert!(!html.contains("code="));
        assert!(html.contains("viewBox"));
    }
}
