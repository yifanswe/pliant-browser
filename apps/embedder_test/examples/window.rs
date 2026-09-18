//! Opens one plain native window; close the window to stop the engine.
use pliant_embedder::Event;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "https://example.com".to_owned());
    let mut failure = None;
    pliant_embedder::run(|engine, event| {
        let result = match event {
            Event::Ready => engine.create_page(&url).map(|_| ()),
            Event::Closed { .. } => engine.shutdown(),
            Event::CloseRefused { .. } => {
                println!("Close refused by the page's beforeunload handler");
                Ok(())
            }
            Event::Navigated { url, .. } => {
                println!("Navigated: {url}");
                Ok(())
            }
            Event::Painted { url, .. } => {
                println!("Visible content painted: {url}");
                Ok(())
            }
            Event::NavigationFailed { code, .. } => Err(pliant_embedder::Error::Native(code)),
            Event::RendererFailed { .. } => Err(pliant_embedder::Error::CreationFailed),
        };
        if let Err(error) = result {
            failure = Some(error);
            let _ = engine.shutdown();
        }
    })?;
    if let Some(error) = failure {
        return Err(error.into());
    }
    Ok(())
}
