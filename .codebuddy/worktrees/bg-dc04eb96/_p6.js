// 设计稿对齐：提交山形图、tab 类型图标、远程卡片 URL
const fs = require('fs');

// ---------- src/gitops.rs ----------
{
  const f = 'D:/BT/devview/src/gitops.rs';
  let s = fs.readFileSync(f, 'utf8');
  let ok = 0;
  function rep(old, nw) {
    const n = s.split(old).length - 1;
    if (n !== 1) { console.error('gitops FAIL(' + n + '): ' + old.slice(0, 50)); process.exit(1); }
    s = s.replace(old, nw); ok++;
  }
  rep(`    pub remote: String,`,
`    pub remote: String,
    pub remote_url: String,`);
  rep(`    if let Some(r) = run(root, &["remote"], 3000) {
        info.remote = r.lines().next().unwrap_or("").trim().to_string();
    }

    // 与上游的差距；没有 upstream 时静默为 0`,
`    if let Some(r) = run(root, &["remote"], 3000) {
        info.remote = r.lines().next().unwrap_or("").trim().to_string();
    }
    if let Some(u) = run(root, &["config", "--get", "remote.origin.url"], 3000) {
        info.remote_url = u.trim().to_string();
    }

    // 与上游的差距；没有 upstream 时静默为 0`);
  rep(`    }    if let Some(r) = run(root, &["remote"], 3000) {
        info.remote = r.lines().next().unwrap_or("").trim().to_string();
    }    // 与上游的差距；没有 upstream 时静默为 0`,
`    }    if let Some(r) = run(root, &["remote"], 3000) {
        info.remote = r.lines().next().unwrap_or("").trim().to_string();
    }
    if let Some(u) = run(root, &["config", "--get", "remote.origin.url"], 3000) {
        info.remote_url = u.trim().to_string();
    }    // 与上游的差距；没有 upstream 时静默为 0`);
  fs.writeFileSync(f, s);
  console.log('OK gitops.rs ' + ok);
}

// ---------- src/app.rs ----------
{
  const f = 'D:/BT/devview/src/app.rs';
  let s = fs.readFileSync(f, 'utf8');
  let ok = 0;
  function rep(old, nw) {
    const n = s.split(old).length - 1;
    if (n !== 1) { console.error('app.rs FAIL(' + n + '): ' + old.slice(0, 70).replace(/\n/g, '\\n')); process.exit(1); }
    s = s.replace(old, nw); ok++;
  }

  // 1. 提交山形图：基线圆点链 + 本地领先时中段拱起曲线（设计稿样式）
  rep(`fn graph(ui: &mut egui::Ui, ahead: usize, behind: usize) -> egui::Response {
    let w = 132.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 32.0), Sense::click());
    let p = ui.painter();
    let y = rect.center().y;
    p.line_segment(
        [
            egui::pos2(rect.left() + 6.0, y),
            egui::pos2(rect.right() - 6.0, y),
        ],
        Stroke::new(1.4, theme::line2()),
    );
    let n = (ahead + behind + 4).min(7);
    for i in 0..n {
        let t = (i as f32 + 1.0) / (n as f32 + 1.0);
        let x = rect.left() + 6.0 + t * (w - 12.0);
        let c = if behind > 0 && i + 1 > n - behind {
            theme::jade()
        } else {
            theme::brass()
        };
        p.circle_filled(egui::pos2(x, y), 3.0, c);
    }
    let c = egui::pos2(rect.right() - 6.0, y);
    p.circle(c, 4.0, theme::bg2(), Stroke::new(1.4, theme::brass()));
    resp
}`,
`fn graph(ui: &mut egui::Ui, ahead: usize, behind: usize) -> egui::Response {
    let w = 132.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 32.0), Sense::click());
    let p = ui.painter();
    let y = rect.center().y;
    p.line_segment(
        [
            egui::pos2(rect.left() + 6.0, y),
            egui::pos2(rect.right() - 6.0, y),
        ],
        Stroke::new(1.4, theme::line2()),
    );
    let n = 5usize;
    let seg = (w - 12.0) / (n as f32 + 1.0);
    for i in 0..n {
        let x = rect.left() + 6.0 + (i as f32 + 1.0) * seg;
        let c = if behind > 0 && i >= n.saturating_sub(behind.min(n)) {
            theme::jade()
        } else {
            theme::brass()
        };
        p.circle_filled(egui::pos2(x, y), 3.0, c);
    }
    // 本地领先：中段两点间拱起一条山形曲线，顶点 jade（设计稿样式）
    if ahead > 0 {
        let apex = (n / 2).max(1) as f32;
        let x0 = rect.left() + 6.0 + apex * seg;
        let x1 = rect.left() + 6.0 + (apex + 2.0) * seg;
        let xm = (x0 + x1) / 2.0;
        let top = y - 10.0;
        let pts = vec![
            egui::pos2(x0, y),
            egui::pos2((x0 + xm) / 2.0, y - 4.5),
            egui::pos2(xm, top),
            egui::pos2((xm + x1) / 2.0, y - 4.5),
            egui::pos2(x1, y),
        ];
        p.add(egui::Shape::line(pts, Stroke::new(1.4, theme::jade())));
        p.circle_filled(egui::pos2(xm, top), 3.0, theme::jade());
    }
    let c = egui::pos2(rect.right() - 6.0, y);
    p.circle(c, 4.0, theme::bg2(), Stroke::new(1.4, theme::brass()));
    resp
}`);

  // 2. tab 类型图标 + 替换 tab 内固定方框
  rep(`/// 设计稿风格的开关：胶囊轨道 + 滑块圆点，选中态黄铜色。`,
`/// 按文件类型绘制的 tab 小图标（设计稿：代码=框、md=文档折角、图片=山形图）。
fn file_icon(p: &egui::Painter, c: egui::Pos2, name: &str) {
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    let st = Stroke::new(1.0, theme::faint());
    let b = egui::Rect::from_center_size(c, Vec2::new(9.0, 11.0));
    match ext.as_str() {
        "md" => {
            // 文档折角：矩形右上斜切 + 两道文字线
            p.add(egui::Shape::convex_polygon(
                vec![
                    b.left_top(),
                    egui::pos2(b.right() - 3.0, b.top()),
                    egui::pos2(b.right(), b.top() + 3.0),
                    b.right_bottom(),
                    b.left_bottom(),
                ],
                Color32::TRANSPARENT,
                st,
            ));
            p.line_segment(
                [egui::pos2(b.left() + 2.0, b.top() + 4.5), egui::pos2(b.right() - 3.5, b.top() + 4.5)],
                st,
            );
            p.line_segment(
                [egui::pos2(b.left() + 2.0, b.top() + 7.0), egui::pos2(b.right() - 2.0, b.top() + 7.0)],
                st,
            );
        }
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg" | "ico" => {
            // 图片：框 + 太阳 + 山形
            p.rect_stroke(b, 1.5, st);
            p.circle_filled(egui::pos2(b.left() + 3.0, b.top() + 3.5), 1.2, theme::brass());
            p.add(egui::Shape::line(
                vec![
                    egui::pos2(b.left() + 1.5, b.bottom() - 2.5),
                    egui::pos2(b.left() + 4.0, b.top() + 5.5),
                    egui::pos2(b.center().x, b.bottom() - 4.0),
                    egui::pos2(b.right() - 1.5, b.bottom() - 2.5),
                ],
                st,
            ));
        }
        _ => {
            // 代码：方框 + 黄铜左条
            p.rect_stroke(b, 1.5, st);
            p.rect_filled(
                egui::Rect::from_min_size(
                    egui::pos2(b.left() + 1.5, b.top() + 1.5),
                    Vec2::new(1.6, b.height() - 3.0),
                ),
                0.5,
                theme::brass(),
            );
        }
    }
}

/// 设计稿风格的开关：胶囊轨道 + 滑块圆点，选中态黄铜色。`);

  // tab 里替换原方框图标
  rep(`            let ix = rect.left() + 10.0;
            ui.painter().rect_stroke(
                egui::Rect::from_min_size(
                    egui::pos2(ix, rect.center().y - 5.0),
                    Vec2::new(9.0, 11.0),
                ),
                1.5,
                Stroke::new(1.0, theme::faint()),
            );`,
`            let ix = rect.left() + 10.0;
            file_icon(
                ui.painter(),
                egui::pos2(ix + 4.5, rect.center().y),
                &name,
            );`);

  // 3. 远程卡片：加 URL 行（设计稿 origin + 地址 + 分支）
  rep(`    fn remote_card(&mut self, ui: &mut egui::Ui) {
        let r = ui.max_rect();
        let card = egui::Rect::from_min_max(
            egui::pos2(r.left() + 10.0, r.bottom() - 66.0),
            egui::pos2(r.right() - 10.0, r.bottom() - 8.0),
        );
        let remote = self.git.remote.clone();
        let branch = self.git.branch.clone();
        let p = ui.painter();
        p.rect(card, 9.0, theme::panel2(), Stroke::new(1.0, theme::line()));
        p.text(
            egui::pos2(card.left() + 12.0, card.top() + 17.0),
            Align2::LEFT_CENTER,
            if remote.is_empty() { "origin" } else { remote.as_str() },
            FontId::proportional(11.5),
            theme::text(),
        );
        p.text(
            egui::pos2(card.left() + 12.0, card.top() + 36.0),
            Align2::LEFT_CENTER,
            format!("↑{}   ↓{}", self.git.ahead, self.git.behind),
            FontId::monospace(9.5),
            theme::faint(),
        );
        p.text(
            egui::pos2(card.left() + 12.0, card.top() + 52.0),
            Align2::LEFT_CENTER,
            if branch.is_empty() { "-" } else { branch.as_str() },
            FontId::monospace(9.5),
            theme::brass(),
        );
    }`,
`    fn remote_card(&mut self, ui: &mut egui::Ui) {
        let r = ui.max_rect();
        let card = egui::Rect::from_min_max(
            egui::pos2(r.left() + 10.0, r.bottom() - 78.0),
            egui::pos2(r.right() - 10.0, r.bottom() - 8.0),
        );
        let remote = self.git.remote.clone();
        let url = self.git.remote_url.clone();
        let branch = self.git.branch.clone();
        let p = ui.painter();
        p.rect(card, 9.0, theme::panel2(), Stroke::new(1.0, theme::line()));
        // 第一行：origin 名 + 同步圆点
        p.text(
            egui::pos2(card.left() + 12.0, card.top() + 16.0),
            Align2::LEFT_CENTER,
            if remote.is_empty() { "origin" } else { remote.as_str() },
            FontId::proportional(11.5),
            theme::text(),
        );
        p.circle_filled(egui::pos2(card.right() - 14.0, card.top() + 16.0), 2.5, theme::jade());
        // 第二行：远程地址（去掉协议与 .git 后缀，设计稿样式）
        let mut shown = url;
        for pre in ["https://", "http://", "git@", "ssh://git@"] {
            if let Some(x) = shown.strip_prefix(pre) {
                shown = x.to_string();
            }
        }
        if shown.ends_with(".git") {
            shown.truncate(shown.len() - 4);
        }
        if shown.is_empty() {
            shown = "未配置远程".to_string();
        }
        p.text(
            egui::pos2(card.left() + 12.0, card.top() + 34.0),
            Align2::LEFT_CENTER,
            shown,
            FontId::monospace(9.5),
            theme::faint(),
        );
        // 第三行：本地分支 + 领先落后
        p.text(
            egui::pos2(card.left() + 12.0, card.top() + 52.0),
            Align2::LEFT_CENTER,
            if branch.is_empty() { "-".to_string() } else { branch.clone() },
            FontId::monospace(9.5),
            theme::brass(),
        );
        p.text(
            egui::pos2(card.right() - 12.0, card.top() + 52.0),
            Align2::RIGHT_CENTER,
            format!("↑{} ↓{}", self.git.ahead, self.git.behind),
            FontId::monospace(9.5),
            theme::faint(),
        );
    }`);

  fs.writeFileSync(f, s);
  console.log('OK app.rs ' + ok);
}
console.log('完成');
