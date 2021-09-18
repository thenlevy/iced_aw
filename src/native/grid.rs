//! Use a grid as an input element for creating grids.
//!
//! *This API requires the following crate features to be activated: `grid`*
use std::hash::Hash;

use iced_native::{
    event,
    layout::{Limits, Node},
    Clipboard, Element, Event, Hasher, Layout, Length, Point, Size, Widget,
};

/// A container that distributes its contents in a grid.
///
/// # Example
///
/// ```
/// # use iced_native::{renderer::Null, Text};
/// #
/// # pub type Grid<'a, Message> = iced_aw::native::Grid<'a, Message, Null>;
/// #[derive(Debug, Clone)]
/// enum Message {
/// }
///
/// let grid = Grid::<Message>::with_columns(2)
///     .push(Text::new("First row, first column"))
///     .push(Text::new("First row, second column"))
///     .push(Text::new("Second row, first column"))
///     .push(Text::new("Second row, second column"));
///
/// ```
#[allow(missing_debug_implementations)]
pub struct Grid<'a, Message, Renderer: self::Renderer> {
    /// The distribution [`Strategy`](Strategy) of the [`Grid`](Grid).
    strategy: Strategy,
    /// The elements in the [`Grid`](Grid).
    elements: Vec<Element<'a, Message, Renderer>>,
}

/// The [`Strategy`](Strategy) of how to distribute the columns of the [`Grid`](Grid).
enum Strategy {
    /// Use `n` columns.
    Columns(usize),
    /// Try to fit as much columns that have a fixed width.
    ColumnWidth(u16),
}

impl Default for Strategy {
    fn default() -> Self {
        Self::Columns(1)
    }
}

impl<'a, Message, Renderer> Grid<'a, Message, Renderer>
where
    Renderer: self::Renderer,
{
    /// Creates a new empty [`Grid`](Grid).
    /// Elements will be laid out in a specific amount of columns.
    #[must_use]
    pub fn with_columns(columns: usize) -> Self {
        Self {
            strategy: Strategy::Columns(columns),
            elements: Vec::new(),
        }
    }

    /// Creates a new empty [`Grid`](Grid).
    /// Columns will be generated to fill the given space.
    #[must_use]
    pub fn with_column_width(column_width: u16) -> Self {
        Self {
            strategy: Strategy::ColumnWidth(column_width),
            elements: Vec::new(),
        }
    }

    /// Adds an [`Element`](Element) to the [`Grid`](Grid).
    pub fn push<E>(mut self, element: E) -> Self
    where
        E: Into<Element<'a, Message, Renderer>>,
    {
        self.elements.push(element.into());
        self
    }

    /// Inserts an [`Element`](Element) into the [`Grid`](Grid).
    pub fn insert<E>(&mut self, element: E)
    where
        E: Into<Element<'a, Message, Renderer>>,
    {
        self.elements.push(element.into());
    }
}

impl<'a, Message, Renderer> Widget<Message, Renderer> for Grid<'a, Message, Renderer>
where
    Renderer: self::Renderer,
{
    fn width(&self) -> Length {
        Length::Shrink
    }

    fn height(&self) -> Length {
        Length::Shrink
    }

    fn layout(&self, renderer: &Renderer, limits: &Limits) -> Node {
        if self.elements.is_empty() {
            return Node::new(Size::ZERO);
        }

        match self.strategy {
            // find out how wide a column is by finding the widest cell in it
            Strategy::Columns(columns) => {
                if columns == 0 {
                    return Node::new(Size::ZERO);
                }

                let mut layouts = Vec::with_capacity(self.elements.len());
                let mut column_widths = Vec::<f32>::with_capacity(columns);

                for (column, element) in (0..columns).cycle().zip(&self.elements) {
                    let layout = element.layout(renderer, limits).size();
                    layouts.push(layout);

                    if let Some(column_width) = column_widths.get_mut(column) {
                        *column_width = column_width.max(layout.width);
                    } else {
                        column_widths.insert(column, layout.width);
                    }
                }

                let column_aligns =
                    std::iter::once(&0.)
                        .chain(column_widths.iter())
                        .scan(0., |state, width| {
                            *state += width;
                            Some(*state)
                        });
                let grid_width = column_widths.iter().sum();

                build_grid(columns, column_aligns, layouts.into_iter(), grid_width)
            }
            // find number of columns by checking how many can fit
            Strategy::ColumnWidth(column_width) => {
                let column_limits = limits.width(Length::Units(column_width));
                let column_width: f32 = column_width.into();
                let max_width = limits.max().width;
                let columns = (max_width / column_width).floor() as usize;

                let layouts = self
                    .elements
                    .iter()
                    .map(|element| element.layout(renderer, &column_limits).size());
                let column_aligns =
                    std::iter::successors(Some(0.), |width| Some(width + column_width));
                #[allow(clippy::cast_precision_loss)] // TODO: possible precision loss
                let grid_width = (columns as f32) * column_width;

                build_grid(columns, column_aligns, layouts, grid_width)
            }
        }
    }

    fn on_event(
        &mut self,
        event: Event,
        layout: Layout<'_>,
        cursor_position: Point,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        messages: &mut Vec<Message>,
    ) -> event::Status {
        let children_status: Vec<event::Status> = self
            .elements
            .iter_mut()
            .zip(layout.children())
            .map(|(child, layout)| {
                child.on_event(
                    event.clone(),
                    layout,
                    cursor_position,
                    renderer,
                    clipboard,
                    messages,
                )
            })
            .collect();

        children_status
            .into_iter()
            .fold(event::Status::Ignored, event::Status::merge)
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        defaults: &Renderer::Defaults,
        layout: Layout<'_>,
        cursor_position: Point,
        viewport: &iced_graphics::Rectangle,
    ) -> Renderer::Output {
        renderer.draw(defaults, layout, cursor_position, viewport, &self.elements)
    }

    fn hash_layout(&self, state: &mut Hasher) {
        #[allow(clippy::missing_docs_in_private_items)]
        struct Marker;
        std::any::TypeId::of::<Marker>().hash(state);

        for element in &self.elements {
            element.hash_layout(state);
        }
    }
}

/// Builds the layout of the [`Grid`](grid).
fn build_grid(
    columns: usize,
    column_aligns: impl Iterator<Item = f32> + Clone,
    layouts: impl Iterator<Item = Size> + ExactSizeIterator,
    grid_width: f32,
) -> Node {
    let mut nodes = Vec::with_capacity(layouts.len());
    let mut grid_height = 0.;
    let mut row_height = 0.;

    for ((column, column_align), size) in (0..columns).zip(column_aligns).cycle().zip(layouts) {
        if column == 0 {
            grid_height += row_height;
            row_height = 0.;
        }

        let mut node = Node::new(size);
        node.move_to(Point::new(column_align, grid_height));
        nodes.push(node);
        row_height = row_height.max(size.height);
    }

    grid_height += row_height;

    Node::with_children(Size::new(grid_width, grid_height), nodes)
}

/// The renderer of a [`Grid`](Grid).
///
/// Your render will need to implement this trait before being
/// able to use [`Grid`](Grid) in your user interface.
pub trait Renderer: iced_native::Renderer {
    /// Draws a [`Grid`](Grid).
    ///
    /// In addition to the default parameters, it expects:
    /// - the list of [`Element`](Element)s
    fn draw<Message>(
        &mut self,
        defaults: &Self::Defaults,
        layout: Layout<'_>,
        cursor_position: Point,
        viewport: &iced_graphics::Rectangle,
        elements: &[Element<'_, Message, Self>],
    ) -> Self::Output;
}

#[cfg(debug_assertions)]
impl Renderer for iced_native::renderer::Null {
    fn draw<Message>(
        &mut self,
        _defaults: &Self::Defaults,
        _layout: Layout<'_>,
        _cursor_position: Point,
        _viewport: &iced_graphics::Rectangle,
        _elements: &[Element<'_, Message, Self>],
    ) {
    }
}

impl<'a, Message, Renderer> From<Grid<'a, Message, Renderer>> for Element<'a, Message, Renderer>
where
    Renderer: 'a + self::Renderer,
    Message: 'static,
{
    fn from(grid: Grid<'a, Message, Renderer>) -> Element<'a, Message, Renderer> {
        Element::new(grid)
    }
}
