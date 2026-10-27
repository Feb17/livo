//! 设置页与观看记录页（同原项目“我的”页面下的设置项，只保留桌面端有意义的部分）。
use super::*;
use gpui::AnyElement;
use livo_core::bilibili::QrPoll;

/// 按键名显示成人能读的样子
pub(crate) fn key_label(k: &str) -> String {
    match k {
        "space" => "空格".into(),
        "up" => "↑".into(),
        "down" => "↓".into(),
        "left" => "←".into(),
        "right" => "→".into(),
        "" => "未设置".into(),
        k => k.to_uppercase(),
    }
}

/// "3 分钟前" / "2 小时前" / "5 天前"
fn time_ago(t: u64) -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    match now.saturating_sub(t) {
        d if d < 60 => "刚刚".into(),
        d if d < 3600 => format!("{} 分钟前", d / 60),
        d if d < 86400 => format!("{} 小时前", d / 3600),
        d => format!("{} 天前", d / 86400),
    }
}

impl Livo {
    /// 主题变了：根视图之外，缓存渲染的弹幕面板也要重绘
    pub(crate) fn refresh_theme(&mut self, cx: &mut Context<Self>) {
        if let Some(room) = &self.room {
            room.chat.update(cx, |_, cx| cx.notify());
        }
        cx.notify();
    }

    fn save_and_notify(&mut self, cx: &mut Context<Self>) {
        self.store.save();
        cx.notify();
    }

    fn bili_index(&self) -> Option<usize> {
        self.site_index("bilibili")
    }

    /// 换掉 B站实例，让后续请求带上（或去掉）登录 Cookie
    fn apply_bili_cookie(&mut self) {
        if let Some(i) = self.bili_index() {
            let a = &self.store.bili;
            self.sites[i] = Arc::new(Site::BiliBili(BiliBili::with_cookie(a.cookie.clone(), a.uid)));
        }
    }

    /// 扫码登录：取二维码，之后每 2 秒轮询一次扫码状态
    fn start_bili_login(&mut self, cx: &mut Context<Self>) {
        let Some(i) = self.bili_index() else { return };
        let site = self.sites[i].clone();
        self.login = Some(Login { key: String::new(), qr: vec![], width: 0, status: "正在获取二维码…" });
        cx.notify();
        cx.spawn(async move |this, cx| {
            let site2 = site.clone();
            let generated = rt().spawn(async move {
                let Site::BiliBili(b) = &*site2 else { unreachable!() };
                b.qr_generate().await
            });
            let (url, key) = match generated.await.map_err(anyhow::Error::from).and_then(|r| r) {
                Ok(v) => v,
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.login = None;
                        this.error = Some(e.to_string());
                        cx.notify()
                    })
                    .ok();
                    return;
                }
            };
            let Ok(code) = qrcode::QrCode::new(url.as_bytes()) else { return };
            let qr = code.to_colors().into_iter().map(|c| c == qrcode::Color::Dark).collect();
            let width = code.width();
            let alive = this.update(cx, |this, cx| {
                this.login = Some(Login { key: key.clone(), qr, width, status: "请用哔哩哔哩 App 扫码" });
                cx.notify()
            });
            if alive.is_err() {
                return;
            }
            loop {
                cx.background_executor().timer(Duration::from_secs(2)).await;
                // 面板关掉或换了新二维码就停
                let still = this.update(cx, |this, _| this.login.as_ref().is_some_and(|l| l.key == key)).unwrap_or(false);
                if !still {
                    return;
                }
                let (site2, key2) = (site.clone(), key.clone());
                let polled = rt()
                    .spawn(async move {
                        let Site::BiliBili(b) = &*site2 else { unreachable!() };
                        let state = b.qr_poll(&key2).await?;
                        // 登录成功顺便查一下昵称
                        let user = match &state {
                            QrPoll::Done(cookie) => Some(b.user_info(cookie).await?),
                            _ => None,
                        };
                        anyhow::Ok((state, user))
                    })
                    .await;
                let Ok(Ok((state, user))) = polled else { continue };
                let finished = this
                    .update(cx, |this, cx| {
                        let Some(login) = this.login.as_mut() else { return true };
                        let done = match state {
                            QrPoll::Waiting => false,
                            QrPoll::Scanned => {
                                login.status = "已扫码，请在手机上确认";
                                false
                            }
                            QrPoll::Expired => {
                                login.status = "二维码已过期，请重新获取";
                                true
                            }
                            QrPoll::Done(cookie) => {
                                let (uid, name) = user.unwrap_or_default();
                                this.store.bili = store::BiliAccount { cookie, uid, name };
                                this.store.save();
                                this.apply_bili_cookie();
                                this.login = None;
                                true
                            }
                        };
                        cx.notify();
                        done
                    })
                    .unwrap_or(true);
                if finished {
                    return;
                }
            }
        })
        .detach();
    }

    fn logout_bili(&mut self, cx: &mut Context<Self>) {
        self.store.bili = Default::default();
        self.apply_bili_cookie();
        self.save_and_notify(cx);
    }

    /// 主页设置：上移/下移
    fn move_home(&mut self, idx: usize, up: bool, cx: &mut Context<Self>) {
        let to = if up { idx.checked_sub(1) } else { Some(idx + 1) };
        if let Some(to) = to.filter(|&t| t < self.store.home_sort.len()) {
            self.store.home_sort.swap(idx, to);
            self.save_and_notify(cx);
        }
    }

    fn move_site(&mut self, idx: usize, up: bool, cx: &mut Context<Self>) {
        let to = if up { idx.checked_sub(1) } else { Some(idx + 1) };
        let Some(to) = to.filter(|&t| t < self.sites.len()) else { return };
        let current = self.sites[self.site_idx].id();
        self.sites.swap(idx, to);
        // 当前选中的平台不随排序变
        self.site_idx = self.site_index(current).unwrap_or(0);
        self.store.site_sort = self.sites.iter().map(|s| s.id().to_string()).collect();
        self.save_and_notify(cx);
    }

    // ---------- 通用控件 ----------

    fn section(&self, title: &'static str, rows: Vec<AnyElement>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().px_1().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(rgb(muted())).child(title))
            .child(div().flex().flex_col().rounded_xl().bg(rgb(surface())).border_1().border_color(rgb(border())).children(rows))
    }

    fn row(&self, title: &'static str, subtitle: Option<&'static str>, control: impl IntoElement) -> AnyElement {
        div()
            .min_h(px(56.))
            .px_4()
            .py_2()
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .border_b_1()
            .border_color(rgb(border()))
            .child(div().flex().flex_col().gap_0p5().min_w(px(0.)).child(div().text_sm().child(title)).children(subtitle.map(|s| div().text_xs().text_color(rgb(muted())).child(s))))
            .child(control)
            .into_any_element()
    }

    /// 开关（macOS 风格）：点击后拖柄滑过去、轨道颜色渐变
    fn switch(&self, id: &'static str, on: bool, set: fn(&mut Self, bool), cx: &mut Context<Self>) -> impl IntoElement {
        let accent = self.accent();
        // p：0 关、1 开，轨道颜色和拖柄位置都按它插值
        let track = move |p: f32| {
            div()
                .size_full()
                .relative()
                .rounded_full()
                .bg(rgb(hover()))
                .child(div().absolute().inset_0().rounded_full().bg(rgb(accent)).opacity(p))
                .child(div().absolute().top(px(2.)).left(px(2. + 18. * p)).size(px(18.)).rounded_full().bg(rgb(0xffffff)).shadow_sm())
        };
        let to = if on { 1. } else { 0. };
        let body = match self.anim_gen(id) {
            Some(gen) => track(1. - to).with_animation((id, gen), Animation::new(Duration::from_millis(200)).with_easing(ease_in_out), move |_, t| track(1. - to + (2. * to - 1.) * t)).into_any_element(),
            None => track(to).into_any_element(),
        };
        div().id(id).w(px(40.)).h(px(22.)).flex_none().cursor_pointer().child(body).on_click(cx.listener(move |this, _, _, cx| {
            this.touch(id);
            set(this, !on);
            this.save_and_notify(cx)
        }))
    }

    /// 分段控件（macOS 风格）：灰色轨道上浮起一块选中背景，刚点的那一下淡入
    fn choices(&self, id: &'static str, options: &[&'static str], selected: usize, set: fn(&mut Self, usize), cx: &mut Context<Self>) -> impl IntoElement {
        let anim = self.anim_gen(id);
        div().flex().flex_none().gap_0p5().p(px(2.)).rounded_lg().bg(rgb(hover())).children(options.iter().enumerate().map(|(i, label)| {
            let on = i == selected;
            let b = div().id(SharedString::from(format!("{id}-{i}"))).relative().px_3().py_1().rounded_md().text_xs().cursor_pointer();
            let b = if on {
                let thumb = div().absolute().inset_0().rounded_md().bg(rgb(raised())).shadow_sm();
                let thumb = match anim {
                    Some(gen) => thumb.with_animation((id, gen), Animation::new(Duration::from_millis(180)).with_easing(ease_out_quint()), |d, t| d.opacity(t)).into_any_element(),
                    None => thumb.into_any_element(),
                };
                b.text_color(rgb(text())).font_weight(FontWeight::MEDIUM).child(thumb)
            } else {
                b.text_color(rgb(muted())).hover(|s| s.text_color(rgb(text())))
            };
            // 文字要盖在浮起的背景上面
            b.child(div().relative().child(*label)).on_click(cx.listener(move |this, _, _, cx| {
                this.touch(id);
                set(this, i);
                this.save_and_notify(cx)
            }))
        }))
    }

    fn small_btn(&self, id: impl Into<SharedString>, label: impl Into<SharedString>) -> gpui::Stateful<gpui::Div> {
        div().id(id.into()).px_3().py_1().rounded_md().cursor_pointer().text_xs().bg(rgb(hover())).hover(|s| s.bg(rgb(border()))).child(label.into())
    }

    fn sort_list(&self, id: &'static str, items: Vec<String>, first_note: Option<&'static str>, mv: fn(&mut Self, usize, bool, &mut Context<Self>), cx: &mut Context<Self>) -> impl IntoElement {
        let n = items.len();
        div().flex().flex_col().gap_1().children(items.into_iter().enumerate().map(|(i, label)| {
            let arrow = |dir: &'static str, up: bool, enabled: bool| {
                let b = div().id(SharedString::from(format!("{id}-{i}-{dir}"))).size(px(26.)).rounded_md().flex().items_center().justify_center().text_sm();
                if enabled {
                    b.cursor_pointer().bg(rgb(hover())).hover(|s| s.bg(rgb(border()))).on_click(cx.listener(move |this, _, _, cx| mv(this, i, up, cx)))
                } else {
                    b.text_color(rgb(subtle()))
                }
                .child(dir)
            };
            div()
                .flex()
                .items_center()
                .gap_2()
                .w(px(260.))
                .child(div().w(px(18.)).text_xs().text_color(rgb(subtle())).child(format!("{}", i + 1)))
                .child(div().flex_1().text_sm().child(label))
                .children((i == 0).then_some(first_note).flatten().map(|t| div().text_xs().text_color(rgb(muted())).child(t)))
                .child(arrow("↑", true, i > 0))
                .child(arrow("↓", false, i + 1 < n))
        }))
    }

    // ---------- 页面 ----------

    pub(crate) fn render_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let st = &self.store;
        let accent = self.accent();

        // 账号管理
        let bili_control = match (&self.login, st.bili.cookie.is_empty()) {
            (Some(_), _) => self.small_btn("bili-cancel", "取消").on_click(cx.listener(|this, _, _, cx| {
                this.login = None;
                cx.notify()
            })),
            (None, true) => self.small_btn("bili-login", "扫码登录").on_click(cx.listener(|this, _, _, cx| this.start_bili_login(cx))),
            (None, false) => self.small_btn("bili-logout", "退出登录").on_click(cx.listener(|this, _, _, cx| this.logout_bili(cx))),
        };
        let bili_sub = if st.bili.cookie.is_empty() { "未登录，登录后可观看更高清晰度" } else { "已登录" };
        let mut account = vec![self.row("哔哩哔哩", Some(bili_sub), div().flex().items_center().gap_3().children((!st.bili.name.is_empty()).then(|| div().text_sm().text_color(rgb(accent)).child(st.bili.name.clone()))).child(bili_control))];
        if let Some(login) = &self.login {
            // 二维码画成小方格；白底留边方便识别
            let cell = 4.;
            let qr = div().p_3().bg(rgb(0xffffff)).rounded_lg().flex().flex_col().children(login.qr.chunks(login.width.max(1)).map(|row| div().flex().children(row.iter().map(|&dark| div().size(px(cell)).bg(rgb(if dark { 0x000000 } else { 0xffffff }))))));
            account.push(div().p_4().flex().items_center().gap_4().border_b_1().border_color(rgb(border())).children((!login.qr.is_empty()).then_some(qr)).child(div().text_sm().text_color(rgb(muted())).child(login.status)).into_any_element());
        }
        account.push(self.row("斗鱼直播", Some("暂不支持登录，匿名可正常观看"), div()));
        account.push(self.row("虎牙直播", Some("无需登录"), div()));

        // 外观设置
        let theme_idx = match st.theme {
            Theme::System => 0,
            Theme::Light => 1,
            Theme::Dark => 2,
        };
        let appearance = vec![self.row(
            "主题",
            None,
            self.choices(
                "theme",
                &["跟随系统", "浅色模式", "深色模式"],
                theme_idx,
                |t, i| t.store.theme = [Theme::System, Theme::Light, Theme::Dark][i],
                cx,
            ),
        )];

        // 主页设置
        let home_items = st.home_sort.iter().filter_map(|id| HOME_PAGES.iter().find(|(p, _)| p == id).map(|(_, n)| n.to_string())).collect();
        let site_items = self.sites.iter().map(|s| s.name().to_string()).collect();
        let home = vec![
            self.row("页面顺序", Some("侧边栏“浏览”的顺序，排第一的是启动页"), self.sort_list("home", home_items, Some("启动页"), |t, i, up, cx| t.move_home(i, up, cx), cx)),
            self.row("平台顺序", None, self.sort_list("sites", site_items, None, |t, i, up, cx| t.move_site(i, up, cx), cx)),
        ];

        // 直播设置
        let lv = &st.live;
        let buffers = [16u32, 32, 64, 128];
        let live = vec![
            self.row("默认清晰度", None, self.choices("quality", &["最低", "中等", "最高", "记住上次"], lv.quality_level.min(3) as usize, |t, i| t.store.live.quality_level = i as u8, cx)),
            self.row("硬件解码", Some("关闭后改用软件解码，下次播放生效"), self.switch("hwdec", lv.hwdec, |t, v| t.store.live.hwdec = v, cx)),
            self.row(
                "缓冲区大小",
                Some("若播放卡顿可尝试调高，下次播放生效"),
                self.choices("buffer", &["16MB", "32MB", "64MB", "128MB"], buffers.iter().position(|b| *b == lv.buffer_mb).unwrap_or(0), |t, i| t.store.live.buffer_mb = [16, 32, 64, 128][i], cx),
            ),
            self.row(
                "画面尺寸",
                None,
                self.choices("fit", &["适应", "填充", "拉伸"], lv.fit.min(2) as usize, |t, i| t.store.live.fit = i as u8, cx),
            ),
            self.row("使用HTTPS链接", Some("将 http 链接替换为 https"), self.switch("https", lv.https, |t, v| t.store.live.https = v, cx)),
            self.row("进入直播间自动全屏", None, self.switch("auto-full", lv.auto_fullscreen, |t, v| t.store.live.auto_fullscreen = v, cx)),
        ];

        // 弹幕设置
        let danmaku = vec![
            self.row("默认开启弹幕", Some("进入直播间时是否显示滚动弹幕"), self.switch("dm-enabled", st.danmaku.enabled, |t, v| t.store.danmaku.enabled = v, cx)),
            div().px_1().py_2().border_b_1().border_color(rgb(border())).child(self.render_danmaku_settings(cx)).into_any_element(),
        ];

        // 播放页设置：快捷键
        let mut keys: Vec<AnyElement> = KEY_ACTIONS
            .iter()
            .map(|&(action, label, _)| {
                let capturing = self.capturing == Some(action);
                let chip = div()
                    .id(SharedString::from(format!("key-{action}")))
                    .min_w(px(64.))
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .text_xs()
                    .text_center()
                    .cursor_pointer()
                    .border_1()
                    .when(capturing, |d| d.border_color(rgb(accent)).text_color(rgb(accent)))
                    .when(!capturing, |d| d.border_color(rgb(border())).bg(rgb(hover())))
                    .child(if capturing { "按下新按键…".to_string() } else { key_label(&st.key_for(action)) })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.capturing = Some(action);
                        cx.notify()
                    }));
                self.row(label, None, chip)
            })
            .collect();
        keys.push(self.row(
            "恢复默认快捷键",
            Some("点击按键后按下新的按键即可修改，Esc 取消；仅在直播间生效"),
            self.small_btn("keys-reset", "恢复默认").on_click(cx.listener(|this, _, _, cx| {
                this.store.keys = Store::default().keys;
                this.save_and_notify(cx)
            })),
        ));

        // 关注设置
        let fs = st.follow;
        let intervals = [5u32, 10, 15, 30, 60];
        let threads = [1u32, 2, 4, 8];
        let follow = vec![
            self.row("自动更新关注直播状态", None, self.switch("follow-auto", fs.auto_refresh, |t, v| t.store.follow.auto_refresh = v, cx)),
            self.row(
                "自动更新间隔",
                None,
                self.choices("follow-interval", &["5分钟", "10分钟", "15分钟", "30分钟", "60分钟"], intervals.iter().position(|v| *v == fs.interval_min).unwrap_or(1), |t, i| t.store.follow.interval_min = [5, 10, 15, 30, 60][i], cx),
            ),
            self.row(
                "更新线程数",
                Some("同时查询的房间数"),
                self.choices("follow-threads", &["1", "2", "4", "8"], threads.iter().position(|v| *v == fs.threads).unwrap_or(2), |t, i| t.store.follow.threads = [1, 2, 4, 8][i], cx),
            ),
        ];

        // 定时关闭
        let ae = st.auto_exit;
        let minutes = [15u32, 30, 60, 90, 120];
        let auto_exit = vec![
            self.row("启用定时关闭", None, self.switch("exit-on", ae.enabled, |t, v| t.store.auto_exit.enabled = v, cx)),
            self.row(
                "自动关闭时间",
                Some("从进入直播间开始倒计时，到时退出应用"),
                self.choices("exit-min", &["15分钟", "30分钟", "1小时", "1.5小时", "2小时"], minutes.iter().position(|v| *v == ae.minutes).unwrap_or(2), |t, i| t.store.auto_exit.minutes = [15, 30, 60, 90, 120][i], cx),
            ),
        ];

        div()
            .max_w(px(760.))
            .flex()
            .flex_col()
            .gap_6()
            .child(self.section("账号管理", account))
            .child(self.section("外观设置", appearance))
            .child(self.section("主页设置", home))
            .child(self.section("直播设置", live))
            .child(self.section("弹幕设置", danmaku))
            .child(self.section("播放页设置", keys))
            .child(self.section("关注设置", follow))
            .child(self.section("定时关闭", auto_exit))
    }

    pub(crate) fn render_history(&self, width: f32, cx: &mut Context<Self>) -> impl IntoElement {
        if self.store.history.is_empty() {
            return div().pt(px(120.)).flex().justify_center().text_color(rgb(muted())).child("还没有观看记录").into_any_element();
        }
        let cols = ((width + GAP) / (MIN_CARD + GAP)).floor().max(1.);
        let card_w = ((width - GAP * (cols - 1.)) / cols - 0.5).floor();
        let cards = self.store.history.iter().enumerate().map(|(i, h)| {
            let site = self.site_index(&h.site).map_or("", |s| self.sites[s].name());
            let (site_id, room_id) = (h.site.clone(), h.room_id.clone());
            // 删除按钮浮在封面右上角，悬停卡片时出现
            let delete = div()
                .id(SharedString::from(format!("history-del-{i}")))
                .absolute()
                .top(px(8.))
                .right(px(8.))
                .size(px(24.))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(tint(0x000000, 0xa0))
                .text_color(rgb(0xffffff))
                .text_xs()
                .cursor_pointer()
                .opacity(0.)
                .group_hover("card", |s| s.opacity(1.))
                .child("✕")
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.store.history.remove(i);
                    this.save_and_notify(cx);
                    cx.stop_propagation()
                }));
            div()
                .relative()
                .child(
                    self.card(SharedString::from(format!("history-{}-{}", h.site, h.room_id)), card_w, &h.cover, &h.title, &format!("{} · {site} · {}", h.user_name, time_ago(h.time)), Some(String::new()), site)
                        .on_click(cx.listener(move |this, _, _, cx| this.open_on(&site_id, room_id.clone(), cx)))
                        .child(delete),
                )
        });
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(div().flex().child(self.small_btn("history-clear", "清空记录").on_click(cx.listener(|this, _, _, cx| {
                this.store.history.clear();
                this.save_and_notify(cx)
            }))))
            .child(div().flex().flex_wrap().gap(px(GAP)).children(cards))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels() {
        assert_eq!(key_label("space"), "空格");
        assert_eq!(key_label("f"), "F");
        assert_eq!(time_ago(0).ends_with("天前"), true);
    }

    #[test]
    fn default_quality_follows_original() {
        let q: Vec<PlayQuality> = ["原画", "蓝光", "超清", "高清"].iter().map(|n| PlayQuality { name: n.to_string(), qn: 0 }).collect();
        assert_eq!(default_quality(&q, 0, None), 3);
        assert_eq!(default_quality(&q, 1, None), 2);
        assert_eq!(default_quality(&q, 2, None), 0);
        assert_eq!(default_quality(&q, 3, Some("蓝光")), 1);
        assert_eq!(default_quality(&q, 3, Some("不存在")), 2);
    }
}
