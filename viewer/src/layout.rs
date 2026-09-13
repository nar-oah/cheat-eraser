use crate::api::{Answer, ApiClient, Missing, Pages};
use crate::display::{Scene, BORDER};
use anyhow::Result;
use embedded_graphics::prelude::Point;
use std::time::{Duration, Instant};

const SCREEN_SIZE: i32 = 200;
const BOARD_SIZE: i32 = SCREEN_SIZE / 2;
const MARGIN: i32 = 2;
const LENGTH: i32 = BOARD_SIZE - MARGIN * 2;
const ITEM: i32 = (LENGTH - (BORDER as i32) * 2) / 3;
const OFFSET: i32 = ITEM + MARGIN;
const CONTENT_SIZE: usize = 27;
const BOARD_CONTENT_SIZE: usize = 9;
const PAGE_SIZE: usize = 27;
const MULTIPLE_PAGE_SIZE: usize = 3;
const REFRESH_INTERVAL_SECS: u64 = 30;

#[derive(Clone, Copy, Eq, PartialEq)]
enum Step {
    Pages,
    Missing,
    Single,
    Multiple,
    Binary,
    NonChoice,
}

enum NonFrame {
    Text(String),
    Formula(u8),
}

fn get_point(start: Point, index: i32) -> Point {
    let row = (index - 1) / 3 + 1;
    let column = (index - 1) % 3 + 1;
    Point::new(start.x + row * OFFSET, start.y + column * OFFSET)
}

fn split_text(text: &str) -> Vec<String> {
    let chars = text.chars().collect::<Vec<_>>();
    chars
        .chunks(CONTENT_SIZE)
        .map(|chunk| chunk.iter().collect::<String>())
        .collect()
}

struct Chessboard {
    start: Point,
}

impl Chessboard {
    fn new(start: Point) -> Self {
        Self { start }
    }
    fn text_start(&self) -> Point {
        self.start - Point::new(OFFSET / 2, ITEM / 2)
    }
    fn draw_board(&self, scene: &mut Scene) -> Result<()> {
        let base = self.start + Point::new(MARGIN, MARGIN);
        let row = base - Point::new(ITEM, 0);
        scene.add_line(get_point(row, 1), get_point(base, 7))?;
        scene.add_line(get_point(row, 2), get_point(base, 8))?;
        let column = base - Point::new(0, ITEM);
        scene.add_line(get_point(column, 1), get_point(base, 3))?;
        scene.add_line(get_point(column, 4), get_point(base, 6))?;
        Ok(())
    }
    fn draw_cell(&self, scene: &mut Scene, index: usize, text: &str) -> Result<()> {
        scene.add_text(text, get_point(self.text_start(), index as i32))
    }
    fn draw_items(&self, scene: &mut Scene, items: &[String]) -> Result<()> {
        items
            .iter()
            .take(BOARD_CONTENT_SIZE)
            .enumerate()
            .try_for_each(|(index, text)| {
                self.draw_cell(scene, index + 1, text)?;
                Ok::<(), anyhow::Error>(())
            })
    }
    fn draw_link(&self, scene: &mut Scene, index: usize) -> Result<()> {
        let (start, end) = match index {
            1 => (1, 7),
            2 => (2, 8),
            3 => (3, 9),
            4 => (1, 3),
            5 => (4, 6),
            6 => (7, 9),
            7 => (1, 9),
            8 => (7, 3),
            _ => return Ok(()),
        };
        scene.add_line(
            get_point(self.text_start(), start),
            get_point(self.text_start(), end),
        )
    }
    fn draw_location(&self, scene: &mut Scene, mark: &str, number: usize) -> Result<()> {
        self.draw_link(scene, number / 10)?;
        self.draw_items(scene, &location_items(mark, number % 10))
    }
}

fn location_items(mark: &str, unit: usize) -> Vec<String> {
    if unit == 0 {
        return (0..BOARD_CONTENT_SIZE).map(|_| mark.to_string()).collect();
    }
    let mut items = (0..BOARD_CONTENT_SIZE)
        .map(|_| "O".to_string())
        .collect::<Vec<_>>();
    items[0] = mark.to_string();
    (1..unit.min(BOARD_CONTENT_SIZE)).for_each(|index| items[index] = "X".to_string());
    items
}

pub struct Layout<'a> {
    scene: Scene<'a>,
    client: ApiClient,
    pages: Pages,
    missing: Missing,
    answer: Option<Answer>,
    uploaded: bool,
    answer_requested: bool,
    last_refresh: Instant,
    boards: [Chessboard; 4],
    step: Step,
    position: usize,
    frame: usize,
}

impl<'a> Layout<'a> {
    pub fn new(
        scene: Scene<'a>,
        client: ApiClient,
        pages: Pages,
        missing: Missing,
    ) -> Result<Self> {
        let boards = std::array::from_fn(|index| {
            Chessboard::new(Point::new(
                (index as i32 / 2) * BOARD_SIZE,
                (index as i32 % 2) * BOARD_SIZE,
            ))
        });
        let mut layout = Self {
            scene,
            client,
            pages,
            missing,
            answer: None,
            uploaded: false,
            answer_requested: false,
            last_refresh: Instant::now(),
            boards,
            step: Step::Pages,
            position: 0,
            frame: 0,
        };
        layout.render()?;
        Ok(layout)
    }
    pub fn next(&mut self) -> Result<()> {
        if self.answer.is_none() && Self::is_answer_step(self.step) {
            return Ok(());
        }
        if self.step == Step::NonChoice && self.frame + 1 < self.non_frame_len(self.position) {
            self.frame += 1;
            return self.render();
        }
        if self.position + 1 < self.step_len(self.step) {
            self.position += 1;
            self.frame = 0;
            return self.render();
        }
        if let Some(step) = self.next_step() {
            if self.step == Step::Missing && step == Step::Single {
                self.prepare_answer()?;
            }
            self.step = step;
            self.position = 0;
            self.frame = 0;
            self.render()?;
        }
        Ok(())
    }
    pub fn previous(&mut self) -> Result<()> {
        if self.step == Step::NonChoice && self.frame > 0 {
            self.frame -= 1;
            return self.render();
        }
        if self.position > 0 {
            self.position -= 1;
            self.frame = self.non_frame_len(self.position).saturating_sub(1);
            return self.render();
        }
        if let Some(step) = self.previous_step() {
            self.step = step;
            self.position = self.step_len(step).saturating_sub(1);
            self.frame = self.non_frame_len(self.position).saturating_sub(1);
            self.render()?;
        }
        Ok(())
    }
    pub fn reset(&mut self) -> Result<()> {
        self.step = Step::Pages;
        self.position = 0;
        self.frame = 0;
        self.answer = None;
        self.uploaded = false;
        self.answer_requested = false;
        self.last_refresh = Instant::now();
        self.render()?;
        self.client.reset()
    }
    pub fn check_sleep(&mut self) -> Result<()> {
        self.scene.check_sleep()
    }
    pub fn refresh_if_due(&mut self) -> Result<()> {
        if self.last_refresh.elapsed() < Duration::from_secs(REFRESH_INTERVAL_SECS) {
            return Ok(());
        }
        self.last_refresh = Instant::now();
        self.refresh_current()
    }
    fn refresh_current(&mut self) -> Result<()> {
        match self.step {
            Step::Pages => {
                self.pages = self.client.get_pages()?;
                self.clamp_position();
                self.render()?;
            }
            Step::Missing => {
                self.missing = self.client.get_missing()?;
                self.clamp_position();
                self.render()?;
            }
            _ if Self::is_answer_step(self.step) && self.uploaded && self.answer.is_none() => {
                if self.refresh_answer()? {
                    self.clamp_position();
                    self.render()?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn prepare_answer(&mut self) -> Result<()> {
        if !self.uploaded {
            self.client.upload()?;
            self.uploaded = true;
        }
        if !self.answer_requested && self.answer.is_none() {
            self.refresh_answer()?;
            self.last_refresh = Instant::now();
        }
        Ok(())
    }
    fn refresh_answer(&mut self) -> Result<bool> {
        self.answer_requested = true;
        if let Some(answer) = self.client.get_answer()? {
            self.answer = Some(answer);
            return Ok(true);
        }
        Ok(false)
    }
    fn clamp_position(&mut self) {
        let len = self.step_len(self.step);
        if self.position >= len {
            self.position = len.saturating_sub(1);
        }
        let frame_len = self.non_frame_len(self.position);
        if self.frame >= frame_len {
            self.frame = frame_len.saturating_sub(1);
        }
    }
    fn is_answer_step(step: Step) -> bool {
        matches!(
            step,
            Step::Single | Step::Multiple | Step::Binary | Step::NonChoice
        )
    }
    fn next_step(&self) -> Option<Step> {
        match self.step {
            Step::Pages => Some(Step::Missing),
            Step::Missing => Some(Step::Single),
            Step::Single => Some(Step::Multiple),
            Step::Multiple => Some(Step::Binary),
            Step::Binary => Some(Step::NonChoice),
            Step::NonChoice => None,
        }
    }
    fn previous_step(&self) -> Option<Step> {
        match self.step {
            Step::Pages => None,
            Step::Missing => Some(Step::Pages),
            Step::Single => Some(Step::Missing),
            Step::Multiple => Some(Step::Single),
            Step::Binary => Some(Step::Multiple),
            Step::NonChoice => Some(Step::Binary),
        }
    }
    fn step_len(&self, step: Step) -> usize {
        match step {
            Step::Pages => self.chunk_len(self.max_page(), PAGE_SIZE),
            Step::Missing => self.chunk_len(self.missing_count(), MULTIPLE_PAGE_SIZE),
            Step::Single => self
                .answer
                .as_ref()
                .map(|answer| self.chunk_len(answer.single_choice.len(), PAGE_SIZE))
                .unwrap_or(1),
            Step::Multiple => self
                .answer
                .as_ref()
                .map(|answer| self.chunk_len(answer.multiple_choice.len(), MULTIPLE_PAGE_SIZE))
                .unwrap_or(1),
            Step::Binary => self
                .answer
                .as_ref()
                .map(|answer| self.chunk_len(answer.binary_choice.len(), PAGE_SIZE))
                .unwrap_or(1),
            Step::NonChoice => self
                .answer
                .as_ref()
                .map(|answer| answer.non_choice.len().max(1))
                .unwrap_or(1),
        }
    }
    fn chunk_len(&self, len: usize, size: usize) -> usize {
        len.saturating_add(size - 1)
            .checked_div(size)
            .unwrap_or(0)
            .max(1)
    }
    fn max_page(&self) -> usize {
        self.pages.iter().copied().max().unwrap_or(1) as usize
    }
    fn missing_count(&self) -> usize {
        self.missing.values().map(Vec::len).sum()
    }
    fn render(&mut self) -> Result<()> {
        self.scene.clear()?;
        self.boards
            .iter()
            .try_for_each(|board| board.draw_board(&mut self.scene))?;
        match self.step {
            Step::Pages => self.render_pages()?,
            Step::Missing => self.render_missing()?,
            Step::Single => self.render_single()?,
            Step::Multiple => self.render_multiple()?,
            Step::Binary => self.render_binary()?,
            Step::NonChoice => self.render_non_choice()?,
        }
        self.scene.refresh()
    }
    fn render_pages(&mut self) -> Result<()> {
        let start = self.position * PAGE_SIZE + 1;
        let max_page = self.max_page();
        let items = (start..=max_page.min(start + PAGE_SIZE - 1))
            .map(|page| {
                if self.pages.contains(&(page as u8)) {
                    "X"
                } else {
                    "O"
                }
                .to_string()
            })
            .collect::<Vec<_>>();
        self.draw_content_items(&items)?;
        self.boards[3].draw_location(&mut self.scene, "页", self.position + 1)
    }
    fn render_missing(&mut self) -> Result<()> {
        let items = self.missing_items();
        items
            .iter()
            .skip(self.position * MULTIPLE_PAGE_SIZE)
            .take(MULTIPLE_PAGE_SIZE)
            .enumerate()
            .try_for_each(|(index, (mark, number))| {
                self.boards[index].draw_location(&mut self.scene, mark, *number)?;
                Ok::<(), anyhow::Error>(())
            })?;
        self.boards[3].draw_location(&mut self.scene, "缺", self.position + 1)
    }
    fn render_answer_pending(&mut self) -> Result<()> {
        self.draw_content_chars("等待答案")?;
        self.boards[3].draw_location(&mut self.scene, "答", 1)
    }
    fn render_single(&mut self) -> Result<()> {
        let items = match &self.answer {
            Some(answer) => answer
                .single_choice
                .iter()
                .skip(self.position * PAGE_SIZE)
                .take(PAGE_SIZE)
                .cloned()
                .collect::<Vec<_>>(),
            None => return self.render_answer_pending(),
        };
        self.draw_content_items(&items)?;
        self.boards[3].draw_location(&mut self.scene, "单", self.position + 1)
    }
    fn render_multiple(&mut self) -> Result<()> {
        let answers = match &self.answer {
            Some(answer) => answer
                .multiple_choice
                .iter()
                .skip(self.position * MULTIPLE_PAGE_SIZE)
                .take(MULTIPLE_PAGE_SIZE)
                .cloned()
                .collect::<Vec<_>>(),
            None => return self.render_answer_pending(),
        };
        answers.iter().enumerate().try_for_each(|(index, answer)| {
            self.boards[index].draw_items(
                &mut self.scene,
                &answer.chars().map(|c| c.to_string()).collect::<Vec<_>>(),
            )?;
            Ok::<(), anyhow::Error>(())
        })?;
        self.boards[3].draw_location(&mut self.scene, "多", self.position + 1)
    }
    fn render_binary(&mut self) -> Result<()> {
        let items = match &self.answer {
            Some(answer) => answer
                .binary_choice
                .iter()
                .skip(self.position * PAGE_SIZE)
                .take(PAGE_SIZE)
                .map(|answer| if *answer { "O" } else { "X" }.to_string())
                .collect::<Vec<_>>(),
            None => return self.render_answer_pending(),
        };
        self.draw_content_items(&items)?;
        self.boards[3].draw_location(&mut self.scene, "判", self.position + 1)
    }
    fn render_non_choice(&mut self) -> Result<()> {
        let answer = match &self.answer {
            Some(answer) => answer,
            None => return self.render_answer_pending(),
        };
        if answer.non_choice.is_empty() {
            return self.boards[3].draw_location(&mut self.scene, "非", 1);
        }
        let word = &answer.non_choice[self.position];
        let answer = word.answer.join("");
        let english = word.english.join(" ");
        let math = word.math;
        let frame = self
            .non_frames(&answer, math)
            .into_iter()
            .nth(self.frame)
            .unwrap_or(NonFrame::Text(String::new()));
        match frame {
            NonFrame::Text(text) => {
                self.draw_content_chars(&text)?;
                self.draw_logo_text(&english)?;
            }
            NonFrame::Formula(index) => {
                self.draw_content_chars("$")?;
                let logo = self.client.get_formula((self.position as u8 + 1, index))?;
                if !logo.is_empty() {
                    self.scene.mod_logo(logo)?;
                }
            }
        }
        self.boards[3].draw_location(&mut self.scene, "非", self.position + 1)
    }
    fn draw_content_items(&mut self, items: &[String]) -> Result<()> {
        self.boards
            .iter()
            .take(3)
            .enumerate()
            .try_for_each(|(board_index, board)| {
                let start = board_index * BOARD_CONTENT_SIZE;
                let end = items.len().min(start + BOARD_CONTENT_SIZE);
                if start < end {
                    board.draw_items(&mut self.scene, &items[start..end])?;
                }
                Ok::<(), anyhow::Error>(())
            })
    }
    fn draw_content_chars(&mut self, text: &str) -> Result<()> {
        let items = text
            .chars()
            .take(CONTENT_SIZE)
            .map(|c| {
                if c == '$' {
                    "■".to_string()
                } else {
                    c.to_string()
                }
            })
            .collect::<Vec<_>>();
        self.draw_content_items(&items)
    }
    fn draw_logo_text(&mut self, text: &str) -> Result<()> {
        split_text(text)
            .iter()
            .take(2)
            .enumerate()
            .try_for_each(|(index, line)| {
                let x = (SCREEN_SIZE - (line.chars().count() as i32 * 12)).max(0) / 2;
                self.scene
                    .add_text(line, Point::new(x, 85 + index as i32 * 16))?;
                Ok::<(), anyhow::Error>(())
            })
    }
    fn missing_items(&self) -> Vec<(String, usize)> {
        let mut keys = self.missing.keys().collect::<Vec<_>>();
        keys.sort();
        keys.iter()
            .flat_map(|key| {
                self.missing[*key]
                    .iter()
                    .map(|number| ((*key).to_string(), *number as usize))
            })
            .collect()
    }
    fn non_frame_len(&self, position: usize) -> usize {
        self.answer
            .as_ref()
            .and_then(|answer| answer.non_choice.get(position))
            .map(|word| self.non_frames(&word.answer.join(""), word.math).len())
            .unwrap_or(1)
            .max(1)
    }
    fn non_frames(&self, answer: &str, math: u8) -> Vec<NonFrame> {
        let parts = answer.split('$').collect::<Vec<_>>();
        let mut frames = parts
            .iter()
            .enumerate()
            .flat_map(|(index, part)| {
                let mut frames = split_text(part)
                    .into_iter()
                    .map(NonFrame::Text)
                    .collect::<Vec<_>>();
                if index + 1 < parts.len() {
                    frames.push(NonFrame::Formula((index + 1) as u8));
                }
                frames
            })
            .collect::<Vec<_>>();
        let formula_count = parts.len().saturating_sub(1);
        (formula_count..math as usize)
            .for_each(|index| frames.push(NonFrame::Formula((index + 1) as u8)));
        if frames.is_empty() {
            frames.push(NonFrame::Text(String::new()));
        }
        frames
    }
}
