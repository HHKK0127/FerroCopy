//! Flexbox — taffy-based layout engine for UI components (Rio-inspired).
//!
//! Calculates positions for UI elements using the taffy flexbox library,
//! producing pixel-perfect rects for egui rendering.

use taffy::prelude::*;

/// A UI element that can be laid out by the flexbox engine.
#[derive(Debug, Clone)]
pub struct FlexItem {
    /// Unique identifier for this item.
    pub id: usize,
    /// Computed x position (pixels).
    pub x: f32,
    /// Computed y position (pixels).
    pub y: f32,
    /// Computed width (pixels).
    pub width: f32,
    /// Computed height (pixels).
    pub height: f32,
}

/// Styles that can be applied to a flex item.
#[derive(Debug, Clone)]
pub struct ItemStyle {
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub min_width: Option<f32>,
    pub min_height: Option<f32>,
    pub margin: [f32; 4],   // [top, right, bottom, left]
    pub flex_grow: f32,
    pub flex_shrink: f32,
}

impl Default for ItemStyle {
    fn default() -> Self {
        Self {
            width: None,
            height: None,
            min_width: None,
            min_height: None,
            margin: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 1.0,
        }
    }
}

/// A flexbox layout container using taffy.
pub struct FlexLayout {
    taffy: TaffyTree<()>,
    root: NodeId,
    items: Vec<(NodeId, ItemStyle)>,
}

impl FlexLayout {
    /// Create a new flexbox layout with the given container dimensions.
    pub fn new(width: f32, height: f32) -> Self {
        let mut taffy = TaffyTree::new();

        let root = taffy
            .new_leaf(Style {
                size: Size {
                    width: Dimension::from_length(width),
                    height: Dimension::from_length(height),
                },
                flex_direction: FlexDirection::Column,
                ..Default::default()
            })
            .unwrap();

        Self {
            taffy,
            root,
            items: Vec::new(),
        }
    }

    /// Add an item with the given style. Returns the item's assigned ID.
    pub fn add_item(&mut self, style: ItemStyle) -> usize {
        let node = self
            .taffy
            .new_leaf(Style {
                size: Size {
                    width: style
                        .width
                        .map_or(Dimension::AUTO, Dimension::from_length),
                    height: style
                        .height
                        .map_or(Dimension::AUTO, Dimension::from_length),
                },
                min_size: Size {
                    width: style
                        .min_width
                        .map_or(Dimension::AUTO, Dimension::from_length),
                    height: style
                        .min_height
                        .map_or(Dimension::AUTO, Dimension::from_length),
                },
                margin: Rect {
                    top: LengthPercentageAuto::from_length(style.margin[0]),
                    right: LengthPercentageAuto::from_length(style.margin[1]),
                    bottom: LengthPercentageAuto::from_length(style.margin[2]),
                    left: LengthPercentageAuto::from_length(style.margin[3]),
                },
                flex_grow: style.flex_grow,
                flex_shrink: style.flex_shrink,
                ..Default::default()
            })
            .unwrap();

        self.taffy.add_child(self.root, node).unwrap();
        let id = self.items.len();
        self.items.push((node, style));
        id
    }

    /// Compute the final positions of all items.
    /// Returns a Vec of `FlexItem` with computed x, y, width, height.
    pub fn compute(&mut self, width: f32, height: f32) -> Vec<FlexItem> {
        self.taffy
            .compute_layout(
                self.root,
                Size {
                    width: AvailableSpace::Definite(width),
                    height: AvailableSpace::Definite(height),
                },
            )
            .unwrap();

        self.items
            .iter()
            .enumerate()
            .map(|(id, (node, _))| {
                let layout = self.taffy.layout(*node).unwrap();
                FlexItem {
                    id,
                    x: layout.location.x,
                    y: layout.location.y,
                    width: layout.size.width,
                    height: layout.size.height,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_column_layout() {
        let mut layout = FlexLayout::new(200.0, 400.0);
        layout.add_item(ItemStyle {
            height: Some(50.0),
            ..Default::default()
        });
        layout.add_item(ItemStyle {
            height: Some(100.0),
            ..Default::default()
        });

        let items = layout.compute(200.0, 400.0);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].width, 200.0);
        assert_eq!(items[0].height, 50.0);
        assert_eq!(items[0].x, 0.0);
        assert_eq!(items[0].y, 0.0);
        assert_eq!(items[1].y, 50.0); // below first item
    }

    #[test]
    fn test_flex_grow_fills_space() {
        let mut layout = FlexLayout::new(300.0, 300.0);
        layout.add_item(ItemStyle {
            height: Some(50.0),
            ..Default::default()
        });
        layout.add_item(ItemStyle {
            flex_grow: 1.0,
            ..Default::default()
        });

        let items = layout.compute(300.0, 300.0);
        assert_eq!(items[1].height, 250.0); // fills remaining space
    }

    #[test]
    fn test_margin_applied() {
        let mut layout = FlexLayout::new(200.0, 100.0);
        layout.add_item(ItemStyle {
            height: Some(50.0),
            margin: [10.0, 0.0, 0.0, 0.0], // top margin
            ..Default::default()
        });

        let items = layout.compute(200.0, 100.0);
        assert_eq!(items[0].y, 10.0); // margin pushes down
    }

    #[test]
    fn test_empty_layout_does_not_panic() {
        let mut layout = FlexLayout::new(100.0, 100.0);
        let items = layout.compute(100.0, 100.0);
        assert!(items.is_empty());
    }

    #[test]
    fn test_multiple_items_order() {
        let mut layout = FlexLayout::new(100.0, 300.0);
        for _ in 0..3 {
            layout.add_item(ItemStyle {
                height: Some(50.0),
                ..Default::default()
            });
        }
        let items = layout.compute(100.0, 300.0);
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].y, 0.0);
        assert_eq!(items[1].y, 50.0);
        assert_eq!(items[2].y, 100.0);
    }
}