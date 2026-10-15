//! 字形光栅化：把字体轮廓转为覆盖率位图。
use super::FontPipeline;

pub(super) const ASCII_START: u32 = 32;
pub(super) const ASCII_END: u32 = 127;

pub(super) const ASCENT_FALLBACK_RATIO: f32 = 0.8;
pub(super) const DESCENT_FALLBACK_RATIO: f32 = 0.2;
pub(super) const CELL_WIDTH_FALLBACK_RATIO: f32 = 0.6;
pub(super) const CELL_HEIGHT_FALLBACK_RATIO: f32 = 1.2;

impl FontPipeline {
    pub fn rasterize_ascii(&mut self) {
        let before = self.cache_length();
        for ch in ASCII_START as u8..ASCII_END as u8 {
            self.glyph_information(ch as char);
        }
        let after = self.cache_length();
        log::debug!(
            "FONT_RASTERIZE_ASCII: before={} after={} font_id={:?}",
            before,
            after,
            self.font_id
        );
    }

    /// 打开主字体面、读度量并按字号换算比例，交给 `use_font` 取所需值。
    ///
    /// `font_id` 缺失、面数据不可读、upem 为 0 三种失败在 `scaled_metric` 与
    /// `cell_metrics` 里走同一批回退值：各写一份「开面 + 算 scale」的样板会让
    /// 回退口径分叉，而分叉只表现为单元格尺寸对不上实际渲染。
    fn with_font_metrics<T>(
        &self,
        use_font: impl FnOnce(swash::FontRef<'_>, swash::Metrics, f32) -> Option<T>,
    ) -> Option<T> {
        let font_id = self.font_id?;
        let font_database = self.font_system.db();
        let result = font_database.with_face_data(font_id, |font_data, face_index| {
            let font_ref = swash::FontRef::from_index(font_data, face_index as usize)?;
            let metrics = font_ref.metrics(&[]);
            let upem = metrics.units_per_em as f32;
            if upem == 0.0 {
                return None;
            }
            use_font(font_ref, metrics, self.font_size / upem)
        });
        result.flatten()
    }

    fn scaled_metric(&self, extract: impl FnOnce(swash::Metrics) -> f32, fallback: f32) -> f32 {
        self.with_font_metrics(|_font_ref, metrics, scale| Some(extract(metrics) * scale))
            .unwrap_or(self.font_size * fallback)
    }

    pub fn ascent_pixels(&self) -> f32 {
        self.scaled_metric(|m| m.ascent, ASCENT_FALLBACK_RATIO)
    }

    pub fn descent_pixels(&self) -> f32 {
        self.scaled_metric(|m| m.descent.abs(), DESCENT_FALLBACK_RATIO)
    }

    pub fn cell_metrics(&self) -> (f32, f32) {
        if let Some(metrics) = self.with_font_metrics(|font_ref, metrics, scale| {
            let ascent = metrics.ascent * scale;
            let descent = metrics.descent.abs() * scale;
            // 行高只取 ascent+descent，不计字体 line gap：Droid Sans Mono 的
            // leading 约 2000 units，即便封顶 25% 仍把行高推到 1.465em，
            // 行间空白达字高的约两倍。line gap 属于段落间距而非每行内部。
            let cell_height = ascent + descent;

            let charmap = font_ref.charmap();
            let glyph_metrics = font_ref.glyph_metrics(&[]);

            if self.font_id.is_some_and(|id| {
                self.font_system
                    .db()
                    .faces()
                    .any(|f| f.id == id && f.monospaced)
            }) {
                let glyph_id = charmap.map('m' as u32);
                let advance = glyph_metrics.advance_width(glyph_id);
                let cell_width = if advance > 0.0 {
                    advance * scale
                } else {
                    self.font_size * CELL_WIDTH_FALLBACK_RATIO
                };
                return Some((cell_width, cell_height.ceil()));
            }

            let max_advance = ['m', 'W', '0']
                .iter()
                .filter_map(|&ch| {
                    let gid = charmap.map(ch as u32);
                    let adv = glyph_metrics.advance_width(gid);
                    if adv > 0.0 { Some(adv * scale) } else { None }
                })
                .fold(0.0f32, f32::max);

            let cell_width = if max_advance > 0.0 {
                max_advance
            } else {
                self.font_size * CELL_WIDTH_FALLBACK_RATIO
            };

            Some((cell_width, cell_height.ceil()))
        }) {
            return metrics;
        }
        (
            self.font_size * CELL_WIDTH_FALLBACK_RATIO,
            (self.font_size * CELL_HEIGHT_FALLBACK_RATIO).ceil(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::FontPipeline;
    use super::{ASCII_END, ASCII_START};

    /// 等宽容差：字体表里的前进宽度是整数 units，换算成像素后同值必然逐位相等，
    /// 这里留一点余量只为吸收 f32 运算顺序差异。
    const ADVANCE_TOLERANCE: f32 = 1e-3;

    /// 真实字体数据当基准：格宽必须是字体自己表里的前进宽度，且该面真的等宽。
    ///
    /// 取的是本仓**实际选中**的主字体面（宿主上来自系统字体库），数据由 swash 直接
    /// 从字体文件读出，与 [`FontPipeline::cell_metrics`] 的计算路径相互独立——upem、
    /// 面索引、advance、ascent/descent 全部出自字体本身：
    ///
    /// - 字体库标注的 `monospaced` 若与表里的实际宽度不符（选错面、或某个
    ///   「等宽」族其实比例宽度），格宽会裁切或空出字形，此前没有任何用例能发现；
    /// - 格宽、行高与字体表换算结果一致，则字号换算链（upem、缩放、上取整）无缺口。
    ///
    /// 字体库为空即判红：没有字体时度量整体退到比例回退值，那正是要防的情况，
    /// 不能当作环境问题放过。
    #[test]
    fn cell_metrics_match_the_selected_face_own_tables() {
        let pipeline = FontPipeline::new(1024, 1024, 14.0);
        let (cell_width, cell_height) = pipeline.cell_metrics();
        let font_id = pipeline
            .font_id
            .expect("宿主字体库应选到主字体面，否则度量已退到比例回退值");
        let database = pipeline.font_system.db();
        let read = database.with_face_data(font_id, |font_data, face_index| {
            let font = swash::FontRef::from_index(font_data, face_index as usize)?;
            let charmap = font.charmap();
            let glyph_metrics = font.glyph_metrics(&[]);
            let metrics = font.metrics(&[]);
            let scale = pipeline.font_size / metrics.units_per_em as f32;
            let advances: Vec<f32> = (ASCII_START..ASCII_END)
                .map(|codepoint| charmap.map(codepoint))
                .filter(|&glyph_id| glyph_id != 0)
                .map(|glyph_id| glyph_metrics.advance_width(glyph_id) * scale)
                .collect();
            Some((
                advances,
                metrics.ascent * scale,
                metrics.descent.abs() * scale,
            ))
        });
        let Some(Some((advances, ascent, descent))) = read else {
            panic!("读取主字体面 {font_id:?} 的字体表失败");
        };
        assert!(
            advances.len() > 90,
            "主字体只覆盖 {} 个可打印 ASCII，取样不足以判定等宽",
            advances.len()
        );

        let mut non_monospaced = Vec::new();
        for (offset, advance) in advances.iter().enumerate() {
            if (advance - advances[0]).abs() > ADVANCE_TOLERANCE {
                non_monospaced.push(format!("U+{:04X}", ASCII_START + offset as u32));
            }
        }
        assert!(
            non_monospaced.is_empty(),
            "选中面被当作等宽，但这些码位的前进宽度与 'm' 不同（{} 个）：{}",
            non_monospaced.len(),
            non_monospaced.join(" ")
        );
        assert!(
            (cell_width - advances[0]).abs() <= ADVANCE_TOLERANCE,
            "格宽 {cell_width} 与字体表前进宽度 {} 不符",
            advances[0]
        );
        assert_eq!(
            cell_height,
            (ascent + descent).ceil(),
            "行高须是 (ascent + descent) 按字号换算后上取整"
        );
    }
}
