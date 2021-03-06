use crate::infobar;
use crate::program::{make_dst, Program};
use crate::ui::{HelpRender, Mode, RotAngle};
use sdl2::image::LoadTexture;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::BlendMode;

const PADDING: i32 = 30;
const HALF_PAD: i32 = 15;
const LINE_HEIGHT: i32 = 22;
const LINE_PADDING: i32 = 5;

struct Colors {
    primary: Color,
    secondary: Color,
    tertiary: Color,
}

impl<'a> Program<'a> {
    /// render_screen is the main render function that delegates rendering every thing that needs be
    /// rendered;
    pub fn render_screen(&mut self, force_render: bool) -> Result<(), String> {
        self.screen.canvas.set_draw_color(dark_grey());
        if self.paths.current_image().is_none() {
            return self.render_blank();
        }
        self.screen.canvas.clear();
        self.render_image(force_render)?;
        if self.ui_state.render_infobar {
            self.render_infobar()?;
        }
        self.render_help()?;

        // Present to screen
        self.screen.canvas.present();
        Ok(())
    }

    fn render_image(&mut self, force_render: bool) -> Result<(), String> {
        self.set_image_texture(force_render)?;
        match self.screen.last_texture {
            Some(_) => (),
            None => return Ok(()),
        };
        let tex = self.screen.last_texture.as_ref().unwrap();
        let query = tex.query();
        // Area to render other rectangle on
        let target = self.screen.canvas.viewport();
        let dst = make_dst(
            &query,
            &target,
            self.ui_state.scale,
            self.ui_state.pan_x,
            self.ui_state.pan_y,
        );

        let angle = match self.ui_state.rot_angle {
            RotAngle::Up => 0.0,
            RotAngle::Right => 90.0,
            RotAngle::Down => 180.0,
            RotAngle::Left => 270.0,
        };
        if let Err(e) = self.screen.canvas.copy_ex(
            &tex,
            None,
            dst,
            angle,
            None,
            self.ui_state.flip_horizontal,
            self.ui_state.flip_vertical,
        ) {
            eprintln!("Failed to copy image to screen {}", e);
        }
        Ok(())
    }

    /// Renders the image at the current index
    fn set_image_texture(&mut self, force_render: bool) -> Result<(), String> {
        if self.paths.index() == self.screen.last_index
            && self.screen.last_texture.is_some()
            && !self.screen.dirty
            && !force_render
        {
            return Ok(());
        }

        //reset horizontal and vertical flip on new image load
        self.ui_state.flip_horizontal = false;
        self.ui_state.flip_vertical = false;

        let current_imagepath = match self.paths.current_image_path() {
            Some(path) => path,
            // No images were found, so no image to load
            None => return Ok(()),
        };

        let texture = match self.screen.texture_creator.load_texture(current_imagepath) {
            Ok(t) => {
                self.screen.last_index = self.paths.index();
                t
            }
            Err(e) => {
                eprintln!("Failed to render image {}", e);
                return Ok(());
            }
        };

        // Set the default state for viewing of the image
        self.screen.last_texture = Some(texture);
        self.screen.dirty = false;
        // fit to screen
        self.ui_state.scale = self.calculate_scale_for_fit();
        // no offsets
        self.ui_state.pan_x = 0.0;
        self.ui_state.pan_y = 0.0;
        // 0 degree rotation
        self.ui_state.rot_angle = RotAngle::Up;
        Ok(())
    }

    /// Computes the default state of actual_size for each image
    pub fn default_actual_size(src_dims: &Rect, dest_dims: &Rect) -> bool {
        // If any dimension of the src image is bigger than the destination
        // dimensions, use scaled size.
        src_dims.x > dest_dims.x || src_dims.y > dest_dims.y
    }

    fn render_infobar(&mut self) -> Result<(), String> {
        let text_color = mode_text_color(&self.ui_state.mode);
        let text = infobar::Text::update(&self.ui_state.mode, &self.paths, &self.ui_state);
        // Load the filename texture
        let filename_surface = self
            .screen
            .font
            .render(&text.information)
            .blended(text_color)
            .map_err(|e| e.to_string())?;
        let filename_texture = self
            .screen
            .texture_creator
            .create_texture_from_surface(&filename_surface)
            .map_err(|e| e.to_string())?;
        let filename_dimensions = filename_texture.query();
        // Load the index texture
        let index_surface = self
            .screen
            .font
            .render(&text.mode)
            .blended(text_color)
            .map_err(|e| e.to_string())?;
        let index_texture = self
            .screen
            .texture_creator
            .create_texture_from_surface(&index_surface)
            .map_err(|e| e.to_string())?;
        let index_dimensions = index_texture.query();
        // Draw the Bar
        let dims = (
            index_dimensions.height,
            index_dimensions.width,
            filename_dimensions.width,
        );
        self.render_bar(dims)?;
        // Copy the text textures
        let y = (self.screen.canvas.viewport().height() - index_dimensions.height) as i32;
        if let Err(e) = self.screen.canvas.copy(
            &index_texture,
            None,
            Rect::new(
                PADDING as i32,
                y,
                index_dimensions.width,
                index_dimensions.height,
            ),
        ) {
            eprintln!("Failed to copy text to screen {}", e);
        }
        if let Err(e) = self.screen.canvas.copy(
            &filename_texture,
            None,
            Rect::new(
                (index_dimensions.width + PADDING as u32 * 2) as i32,
                y,
                filename_dimensions.width,
                filename_dimensions.height,
            ),
        ) {
            eprintln!("Failed to copy text to screen {}", e);
            return Ok(());
        }
        Ok(())
    }

    fn render_help(&mut self) -> Result<(), String> {
        let text = match self.ui_state.render_help {
            HelpRender::None => return Ok(()),
            HelpRender::Normal => normal_help_text(),
            HelpRender::Command => command_help_text(),
        };
        let total_height = LINE_HEIGHT * text.len() as i32 + LINE_PADDING * (text.len() as i32 - 1);
        let mut y = (self.screen.canvas.viewport().height() as f32 / 2.0
            - total_height as f32 / 2.0) as i32;
        let w = {
            let surface = self
                .screen
                .mono_font
                .render(text[0])
                .blended(dark_text_color())
                .map_err(|e| e.to_string())?;
            let texture = self
                .screen
                .texture_creator
                .create_texture_from_surface(&surface)
                .map_err(|e| e.to_string())?;
            texture.query().width
        };
        // Draw the Box
        let dims = (total_height as u32, w);
        self.render_help_box(dims)?;
        // Draw the text
        for line in text {
            // Load the text texture
            let surface = self
                .screen
                .mono_font
                .render(line)
                .blended(dark_text_color())
                .map_err(|e| e.to_string())?;
            let texture = self
                .screen
                .texture_creator
                .create_texture_from_surface(&surface)
                .map_err(|e| e.to_string())?;
            let dimensions = texture.query();
            let x = (self.screen.canvas.viewport().width() as f32 / 2.0
                - dimensions.width as f32 / 2.0) as i32;
            if let Err(e) = self.screen.canvas.copy(
                &texture,
                None,
                Rect::new(x, y, dimensions.width, dimensions.height),
            ) {
                eprintln!("Failed to copy text to screen {}", e);
            } else {
                y += LINE_HEIGHT + LINE_PADDING;
            }
        }
        Ok(())
    }

    fn render_bar(&mut self, dims: (u32, u32, u32)) -> Result<(), String> {
        let colors = mode_colors(&self.ui_state.mode);
        let height = dims.0;
        let width = self.screen.canvas.viewport().width();
        let y = (self.screen.canvas.viewport().height() - height) as i32;
        let mut x = 0;
        let mut w = dims.1 + HALF_PAD as u32 * 3;
        self.screen.canvas.set_draw_color(colors.primary);
        if let Err(e) = self.screen.canvas.fill_rect(Rect::new(x, y, w, height)) {
            eprintln!("Failed to draw bar {}", e);
        }
        x += w as i32;
        w = dims.2 + PADDING as u32 * 2;
        self.screen.canvas.set_draw_color(colors.secondary);
        if let Err(e) = self.screen.canvas.fill_rect(Rect::new(x, y, w, height)) {
            eprintln!("Failed to draw bar {}", e);
        }
        x += w as i32;
        w = width;
        self.screen.canvas.set_draw_color(colors.tertiary);
        if let Err(e) = self.screen.canvas.fill_rect(Rect::new(x, y, w, height)) {
            eprintln!("Failed to draw bar {}", e);
        }
        Ok(())
    }

    fn render_help_box(&mut self, dims: (u32, u32)) -> Result<(), String> {
        let height = dims.0;
        let y = (self.screen.canvas.viewport().height() as f32 / 2.0 - height as f32 / 2.0) as i32;
        let x = (self.screen.canvas.viewport().width() as f32 / 2.0 - dims.1 as f32 / 2.0) as i32;
        let w = dims.1;
        self.screen.canvas.set_draw_color(help_background_color());
        self.screen.canvas.set_blend_mode(BlendMode::Blend);
        if let Err(e) = self.screen.canvas.fill_rect(Rect::new(x, y, w, height)) {
            eprintln!("Failed to draw bar {}", e);
        }
        Ok(())
    }

    fn render_blank(&mut self) -> Result<(), String> {
        self.screen.canvas.clear();
        if self.ui_state.render_infobar {
            self.render_infobar()?;
        }
        self.render_help()?;
        self.screen.canvas.present();
        Ok(())
    }
}

fn mode_colors(m: &Mode) -> Colors {
    match m {
        Mode::Normal | Mode::MultiNormal => Colors {
            primary: light_blue(),
            secondary: blue(),
            tertiary: grey(),
        },
        Mode::Error(_) => Colors {
            primary: light_red(),
            secondary: red(),
            tertiary: grey(),
        },
        Mode::Success(_) => Colors {
            primary: light_green(),
            secondary: green(),
            tertiary: grey(),
        },
        Mode::Command(_) => Colors {
            primary: light_yellow(),
            secondary: yellow(),
            tertiary: grey(),
        },
        Mode::Exit => Colors {
            primary: light_blue(),
            secondary: blue(),
            tertiary: grey(),
        },
    }
}

fn mode_text_color(m: &Mode) -> Color {
    match m {
        Mode::Normal | Mode::MultiNormal | Mode::Exit | Mode::Command(_) | Mode::Success(_) => {
            dark_text_color()
        }
        Mode::Error(_) => light_text_color(),
    }
}

fn dark_grey() -> Color {
    Color::RGB(45, 45, 45)
}

fn dark_text_color() -> Color {
    Color::RGBA(52, 56, 56, 255)
}

fn light_text_color() -> Color {
    Color::RGBA(255, 255, 255, 255)
}

fn help_background_color() -> Color {
    Color::RGBA(0, 223, 252, 200)
}

fn light_blue() -> Color {
    Color::RGB(0, 223, 252)
}

fn blue() -> Color {
    Color::RGB(0, 180, 204)
}

fn light_red() -> Color {
    Color::RGB(252, 45, 45)
}

fn red() -> Color {
    Color::RGB(223, 0, 0)
}

fn light_green() -> Color {
    Color::RGB(45, 252, 45)
}

fn green() -> Color {
    Color::RGB(0, 223, 0)
}

fn light_yellow() -> Color {
    Color::RGB(255, 255, 170)
}

fn yellow() -> Color {
    Color::RGB(255, 255, 130)
}

fn grey() -> Color {
    Color::RGB(52, 56, 56)
}

fn normal_help_text() -> Vec<&'static str> {
    vec![
        "+------------+----------------------------+-----------------------------------------------------+",
        "| Key 1      | Key 2                      | Action                                              |",
        "+------------+----------------------------+-----------------------------------------------------+",
        "| 0-9 (many) | Key1 of action to perform  | Perform the specified action many times             |",
        "| q          | Esc                        | Quit                                                |",
        "| k/j        | Left/Right                 | Previous/Next Image                                 |",
        "| i/o        | Up/Down                    | Zoom in/out                                         |",
        "| r/R        |                            | Rotate image clockwise/counterclockwise             |",
        "| H, J, K, L | Shift + Up/Down/Left/Right | Pan left/down/up/right                              |",
        "| h          |                            | Flip image horizontally                             |",
        "| v          |                            | Flip image vertically                               |",
        "| b/w        | PageDown/PageUp            | Backward/Forward 10% of images                      |",
        "| g/G        | Home/End                   | First/Last Image (55G jumps to the 55th image)      |",
        "| m          |                            | Move image to destination folder (default ./keep)   |",
        "| c          |                            | Copy image to destination folder (default ./keep)   |",
        "| d          | Delete                     | Move image to OS specific trash location            |",
        "| D          | Shift + Delete             | Delete image from its location                      |",
        "| t          |                            | Toggle information bar                              |",
        "| f          | F11                        | Toggle fullscreen mode                              |",
        "| ?          |                            | Toggle help box                                     |",
        "| z          | Left Click                 | Toggle actual size vs scaled image                  |",
        "| Z          |                            | Center image                                        |",
        "| . (period) |                            | Repeat last action                                  |",
        "+------------+----------------------------+-----------------------------------------------------+",
    ]
}

fn command_help_text() -> Vec<&'static str> {
    vec![
        "+-------+------------+----------+-------------------------------------+",
        "| Short | Long       | Argument | Action                              |",
        "+-------+------------+----------+-------------------------------------+",
        "| ng    | newglob    | Required | The new glob/directory/file         |",
        "| ?     | help       | None     | Toggle help box                     |",
        "| q     | quit       | None     | Quit                                |",
        "|       | sort       | Optional | The method to sort by               |",
        "| df    | destfolder | Required | New folder to move/copy images to   |",
        "| m     | max        | Required | New maximum number of files to view |",
        "+---------------------------------------------------------------------+",
        "| Sorting Options                                                     |",
        "+---------------------------------------------------------------------+",
        "| Alphabetical                                                        |",
        "| Date                                                                |",
        "| Size                                                                |",
        "| DepthFirst                                                          |",
        "| BreadthFirst                                                        |",
        "+---------------------------------------------------------------------+",
    ]
}
