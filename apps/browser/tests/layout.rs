use pliant_browser::{LayoutAxis, LayoutSink, render_definition};
use pliant_ui_definition::parse_definition;

#[derive(Default)]
struct RecordingSink {
    nodes: Vec<String>,
}

impl LayoutSink for RecordingSink {
    fn begin(&mut self, generation: u64) -> Result<(), String> {
        self.nodes.push(format!("begin:{generation}"));
        Ok(())
    }

    fn container(
        &mut self,
        parent: Option<&str>,
        id: &str,
        axis: LayoutAxis,
        gap: Option<f64>,
    ) -> Result<(), String> {
        self.nodes
            .push(format!("container:{parent:?}:{id}:{axis:?}:{gap:?}"));
        Ok(())
    }

    fn spacer(&mut self, parent: &str, id: &str, size: f64) -> Result<(), String> {
        self.nodes.push(format!("spacer:{parent}:{id}:{size}"));
        Ok(())
    }

    fn label(&mut self, parent: &str, id: &str, text: &str) -> Result<(), String> {
        self.nodes.push(format!("label:{parent}:{id}:{text}"));
        Ok(())
    }

    fn address_field(
        &mut self,
        parent: &str,
        id: &str,
        placeholder: Option<&str>,
    ) -> Result<(), String> {
        self.nodes
            .push(format!("address:{parent}:{id}:{placeholder:?}"));
        Ok(())
    }

    fn page_list(&mut self, parent: &str, id: &str) -> Result<(), String> {
        self.nodes.push(format!("pages:{parent}:{id}"));
        Ok(())
    }

    fn content_surface(&mut self, parent: &str, id: &str) -> Result<(), String> {
        self.nodes.push(format!("content:{parent}:{id}"));
        Ok(())
    }

    fn button(&mut self, parent: &str, id: &str, label: &str) -> Result<(), String> {
        self.nodes.push(format!("button:{parent}:{id}:{label}"));
        Ok(())
    }

    fn end(&mut self) -> Result<(), String> {
        self.nodes.push("end".to_owned());
        Ok(())
    }
}

#[test]
fn third_definition_is_rendered_by_structure_not_preset_name() {
    let definition = parse_definition(include_str!("../fixtures/third-definition.json")).unwrap();
    let mut sink = RecordingSink::default();

    render_definition(&definition, 9, &mut sink).unwrap();

    assert_eq!(
        sink.nodes,
        [
            "begin:9",
            "container:None:third-root:Row:None",
            "content:third-root:third-content",
            "container:Some(\"third-root\"):third-controls:Column:Some(8.0)",
            "label:third-controls:third-title:Independent JSON layout",
            "address:third-controls:third-address:Some(\"Navigate this page\")",
            "pages:third-controls:third-pages",
            "button:third-controls:third-back:Back",
            "button:third-controls:third-close:Close",
            "end",
        ]
    );
}
