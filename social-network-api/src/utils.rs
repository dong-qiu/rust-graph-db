use rust_graph_db::types::Graphid;

/// Parse a Graphid from a string representation (e.g., "1.100")
pub fn parse_graphid(s: &str) -> anyhow::Result<Graphid> {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 2 {
        anyhow::bail!("Invalid Graphid format: {}", s);
    }

    let label_id: u16 = parts[0].parse()?;
    let vertex_id: u64 = parts[1].parse()?;

    Ok(Graphid::new(label_id, vertex_id)?)
}
