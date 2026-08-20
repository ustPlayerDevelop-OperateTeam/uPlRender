use crate::model::NoteInfo;

#[derive(Debug, Clone)]
pub struct CurrentNote {
    pub note: Option<NoteInfo>,
    pub is_ended: bool,
}

#[derive(Debug, Clone)]
pub struct TimingModel {
    #[allow(dead_code)]
    tempo: f64,
    tick_per_second: f64,
    note_tick_ranges: Vec<(i64, i64, NoteInfo)>,
    total_tick: i64,
}

impl TimingModel {
    pub fn new(tempo: f64, notes: Vec<NoteInfo>) -> Self {
        let mut ranges = Vec::with_capacity(notes.len());
        let mut current_tick: i64 = 0;
        for note in &notes {
            let len = std::cmp::max(note.length, 1) as i64;
            ranges.push((current_tick, current_tick + len, note.clone()));
            current_tick += len;
        }
        // 总时长 = 所有音符长度之和(累加结束后的 current_tick 即总和)
        let total = current_tick;
        Self {
            tempo,
            tick_per_second: tempo * 480.0 / 60.0,
            note_tick_ranges: ranges,
            total_tick: total,
        }
    }

    #[inline]
    pub fn tick_per_second(&self) -> f64 {
        self.tick_per_second
    }

    #[inline]
    pub fn total_tick(&self) -> i64 {
        self.total_tick
    }

    #[inline]
    pub fn total_duration_secs(&self) -> f64 {
        if self.tick_per_second == 0.0 {
            0.0
        } else {
            self.total_tick as f64 / self.tick_per_second
        }
    }

    /// 根据 elapsed_sec 查找当前音符范围；找不到则返回 ended。
    pub fn current_note(&self, elapsed_sec: f64, hint: &mut usize) -> CurrentNote {
        if self.note_tick_ranges.is_empty() {
            *hint = 0;
            return CurrentNote {
                note: None,
                is_ended: true,
            };
        }

        let current_tick = (elapsed_sec * self.tick_per_second) as i64;
        let n = self.note_tick_ranges.len();
        let mut idx = *hint;

        if idx < n {
            let (s, e, _) = self.note_tick_ranges[idx];
            if !(s <= current_tick && current_tick < e) {
                // 向前找
                while idx < n {
                    let (s, e, _) = self.note_tick_ranges[idx];
                    if s <= current_tick && current_tick < e {
                        break;
                    }
                    idx += 1;
                }
                if idx >= n {
                    idx = 0;
                    while idx < *hint {
                        let (s, e, _) = self.note_tick_ranges[idx];
                        if s <= current_tick && current_tick < e {
                            break;
                        }
                        idx += 1;
                    }
                    if idx >= *hint {
                        idx = n;
                    }
                }
            }
        } else {
            idx = n;
        }

        if idx >= n {
            *hint = n;
            return CurrentNote {
                note: None,
                is_ended: true,
            };
        }

        *hint = idx;
        let (_, _, note) = self.note_tick_ranges[idx].clone();
        CurrentNote {
            note: Some(note),
            is_ended: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(length: i32) -> NoteInfo {
        NoteInfo {
            index: String::new(),
            length,
            lyric: String::new(),
            note_num: 60,
            phoneme: String::new(),
            pitch_bend: Vec::new(),
        }
    }

    #[test]
    fn test_tick_per_second() {
        let t = TimingModel::new(120.0, vec![note(480)]);
        assert_eq!(t.tick_per_second(), 960.0); // 120 * 480 / 60
    }

    #[test]
    fn test_total_tick_is_sum_of_lengths() {
        let t = TimingModel::new(120.0, vec![note(480), note(480), note(240)]);
        assert_eq!(t.total_tick(), 1200); // 480+480+240
    }

    #[test]
    fn test_total_tick_clamps_zero_length() {
        let t = TimingModel::new(120.0, vec![note(0), note(-5), note(100)]);
        assert_eq!(t.total_tick(), 102); // 1+1+100
    }

    #[test]
    fn test_total_duration_secs() {
        let t = TimingModel::new(120.0, vec![note(960), note(960)]);
        assert!((t.total_duration_secs() - 2.0).abs() < 1e-9); // 1920 ticks / 960 tps
    }

    #[test]
    fn test_current_note_basic() {
        let t = TimingModel::new(120.0, vec![note(480), note(480)]);
        let mut hint = 0;
        let c0 = t.current_note(0.0, &mut hint);
        assert!(c0.note.is_some());
        assert_eq!(c0.note.unwrap().length, 480);

        // 0.5s = 480 ticks,处于第二个音符 [480, 960)
        let c1 = t.current_note(0.5, &mut hint);
        assert!(c1.note.is_some());
        assert_eq!(hint, 1);

        // 超出总时长 → ended
        let c2 = t.current_note(10.0, &mut hint);
        assert!(c2.note.is_none());
        assert!(c2.is_ended);
    }

    #[test]
    fn test_current_note_gap_between_notes() {
        // 两个音符之间有间隙:长度 480 但 pitch 前后时间不连续?此处验证 tick 是连续排列的,
        // 不存在间隙;time 0.25s=240 ticks 落在第一个音符内。
        let t = TimingModel::new(120.0, vec![note(480), note(480)]);
        let mut hint = 0;
        let c = t.current_note(0.25, &mut hint);
        assert!(c.note.is_some());
        assert_eq!(hint, 0);
    }

    #[test]
    fn test_current_note_time_regression_from_hint() {
        // hint 已推进到 1,时间回退到 0.1s → 应回到第一个音符
        let t = TimingModel::new(120.0, vec![note(480), note(480)]);
        let mut hint = 1;
        let c = t.current_note(0.1, &mut hint);
        assert!(c.note.is_some());
        assert_eq!(hint, 0);
    }

    #[test]
    fn test_empty_notes_is_ended() {
        let t = TimingModel::new(120.0, vec![]);
        let mut hint = 0;
        let c = t.current_note(0.0, &mut hint);
        assert!(c.note.is_none());
        assert!(c.is_ended);
    }
}
