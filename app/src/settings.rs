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
pub(crate) fn time_ago(t: u64) -> String {
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

    /// 设置分组：11px 分组标题 + surface 卡片，行与行之间 1px 分隔
    fn section(&self, title: &'static str, rows: Vec<AnyElement>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(SPACE_2))
            .child(group_label(title))
            .child(div().flex().flex_col().rounded(px(RADIUS_MD)).bg(surface()).border_1().border_color(border()).children(rows.into_iter().enumerate().map(|(i, r)| div().flex().flex_col().when(i > 0, |d| d.border_t_1().border_color(border())).child(r))))
    }

    /// 设置行：左侧名称 + 一行说明，右侧控件
    fn row(&self, title: &'static str, subtitle: Option<&'static str>, control: impl IntoElement) -> AnyElement {
        div()
            .min_h(px(44.))
            .px(px(SPACE_4))
            .py(px(10.))
            .flex()
            .items_center()
            .justify_between()
            .gap(px(SPACE_4))
            .child(div().flex().flex_col().min_w(px(0.)).child(div().t_body().child(title)).children(subtitle.map(|s| div().t_callout().text_color(muted()).child(s))))
            .child(control)
            .into_any_element()
    }

    /// 开关：38 × 22，关闭态轨道 border-strong，打开态 accent-fill；点击后滑块 150ms 滑过去
    fn switch(&self, id: &'static str, on: bool, set: fn(&mut Self, bool), cx: &mut Context<Self>) -> impl IntoElement {
        let (off_c, on_c) = (border_strong(), accent_fill());
        // p：0 关、1 开，轨道颜色和滑块位置都按它插值
        let track = move |p: f32| {
            div()
                .size_full()
                .relative()
                .rounded_full()
                .bg(mix(off_c, on_c, p))
                .child(div().absolute().top(px(2.)).left(px(2. + 16. * p)).size(px(18.)).rounded_full().bg(rgb(0xffffff)).shadow(vec![BoxShadow { color: c(0x00000040).into(), offset: point(px(0.), px(1.)), blur_radius: px(2.), spread_radius: px(0.) }]))
        };
        let to = if on { 1. } else { 0. };
        let body = match self.anim_gen(id) {
            Some(gen) => track(1. - to).with_animation((id, gen), Animation::new(Duration::from_millis(150)).with_easing(ease_out_quint()), move |_, t| track(1. - to + (2. * to - 1.) * t)).into_any_element(),
            None => track(to).into_any_element(),
        };
        div().id(id).w(px(38.)).h(px(22.)).flex_none().cursor_pointer().child(body).on_click(cx.listener(move |this, _, _, cx| {
            this.touch(id);
            set(this, !on);
            this.save_and_notify(cx)
        }))
    }

    /// 分段控件（2–4 个互斥选项，立即生效）：hover 色轨道上浮起一块 surface-raised 选中块，刚点的那一下淡入
    pub(crate) fn seg(&self, id: &'static str, options: Vec<SharedString>, selected: usize, on: impl Fn(&mut Self, usize, &mut Context<Self>) + 'static, cx: &mut Context<Self>) -> impl IntoElement {
        let anim = self.anim_gen(id);
        let on = Rc::new(on);
        div().flex().flex_none().gap(px(2.)).p(px(2.)).rounded(px(RADIUS_SM)).bg(hover()).children(options.into_iter().enumerate().map(|(i, label)| {
            let current = i == selected;
            let b = div().id(SharedString::from(format!("{id}-{i}"))).relative().h(px(24.)).px(px(SPACE_3)).flex().items_center().rounded(px(RADIUS_XS)).t_body().cursor_pointer().focusable_ring();
            let b = if current {
                let thumb = div().absolute().inset_0().rounded(px(RADIUS_XS)).bg(raised()).shadow(shadow_card());
                let thumb = match anim.filter(|_| !reduce_motion()) {
                    Some(gen) => thumb.with_animation((id, gen), Animation::new(Duration::from_millis(180)).with_easing(ease_out_quint()), |d, t| d.opacity(t)).into_any_element(),
                    None => thumb.into_any_element(),
                };
                b.text_color(text()).font_weight(FontWeight::SEMIBOLD).child(thumb)
            } else {
                b.text_color(muted()).hover(|s| s.text_color(text()))
            };
            let on = on.clone();
            // 文字要盖在浮起的背景上面
            b.child(div().relative().child(label)).on_click(cx.listener(move |this, _, _, cx| {
                if i != selected {
                    this.touch(id);
                    on(this, i, cx);
                    cx.notify()
                }
            }))
        }))
    }

    /// 设置里的分段控件：改完立即保存；超过 4 个选项的设置用 `popup` 下拉
    fn choices(&self, id: &'static str, options: &[&'static str], selected: usize, set: fn(&mut Self, usize), cx: &mut Context<Self>) -> impl IntoElement {
        self.seg(id, options.iter().map(|o| SharedString::from(*o)).collect(), selected, move |this, i, cx| {
            set(this, i);
            this.save_and_notify(cx)
        }, cx)
    }

    fn sort_list(&self, id: &'static str, items: Vec<String>, first_note: Option<&'static str>, mv: fn(&mut Self, usize, bool, &mut Context<Self>), cx: &mut Context<Self>) -> impl IntoElement {
        let n = items.len();
        div().flex().flex_col().gap(px(SPACE_1)).children(items.into_iter().enumerate().map(|(i, label)| {
            let arrow = |dir: &'static str, up: bool, enabled: bool| {
                let b = div().id(SharedString::from(format!("{id}-{i}-{dir}"))).size(px(theme::CONTROL_SM)).rounded(px(RADIUS_SM)).flex().items_center().justify_center();
                let glyph = icon("chevron-down", 16., if enabled { muted() } else { subtle() }).when(up, |g| g.with_transformation(gpui::Transformation::rotate(gpui::radians(std::f32::consts::PI))));
                if enabled {
                    b.cursor_pointer().hover(|s| s.bg(hover())).focusable_ring().tooltip(tip(if up { "上移" } else { "下移" }, None)).on_click(cx.listener(move |this, _, _, cx| mv(this, i, up, cx)))
                } else {
                    b.opacity(0.6)
                }
                .child(glyph)
            };
            div()
                .flex()
                .items_center()
                .gap(px(SPACE_2))
                .w(px(240.))
                .child(div().w(px(16.)).t_callout().text_color(subtle()).child(format!("{}", i + 1)))
                .child(div().flex_1().t_body().child(label))
                .children((i == 0).then_some(first_note).flatten().map(|t| badge(Badge::Neutral, None, t)))
                .child(arrow("↑", true, i > 0))
                .child(arrow("↓", false, i + 1 < n))
        }))
    }

    // ---------- 页面 ----------

    /// 超过 4 个选项的设置：弹出按钮显示当前值，点开是打勾的单选菜单（在设置窗口里弹出）
    fn popup(&self, id: &'static str, options: &[&'static str], selected: usize, set: fn(&mut Self, usize), cx: &mut Context<Self>) -> impl IntoElement {
        let options: Vec<&'static str> = options.to_vec();
        popup_button(id, options.get(selected).copied().unwrap_or("")).on_click(cx.listener(move |this, ev: &gpui::ClickEvent, _, cx| {
            let entries = options
                .iter()
                .enumerate()
                .map(|(i, label)| {
                    MenuEntry::radio(*label, i == selected, Rc::new(move |t: &mut Livo, _: &mut Window, cx: &mut Context<Livo>| {
                        set(t, i);
                        t.save_and_notify(cx)
                    }) as Act)
                })
                .collect();
            this.open_ctx(ev.position(), entries, true, cx)
        }))
    }

    /// 快捷键录入框：显示键帽；点击进入录入（2px accent 描边），Esc 取消、⌫ 清除；和别的命令冲突时描边 danger，下方一行说明
    fn key_field(&self, action: &'static str, cx: &mut Context<Self>) -> AnyElement {
        let st = &self.store;
        let key = st.key_for(action);
        let capturing = self.capturing == Some(action);
        let conflict = (!key.is_empty())
            .then(|| KEY_ACTIONS.iter().find(|(a, ..)| *a != action && st.key_for(a) == key).map(|(_, label, _)| *label))
            .flatten();
        let field = div()
            .id(SharedString::from(format!("key-{action}")))
            .min_w(px(120.))
            .h(px(theme::CONTROL_MD))
            .px(px(SPACE_3))
            .flex()
            .items_center()
            .justify_center()
            .gap(px(SPACE_1))
            .rounded(px(RADIUS_SM))
            .bg(surface())
            .cursor_pointer()
            .focusable_ring()
            .map(|d| match (capturing, conflict) {
                (true, _) => d.border_2().border_color(accent()).text_color(accent()).t_body().child("按下新的快捷键…"),
                (false, c) => d.border_1().border_color(if c.is_some() { danger() } else { border_strong() }).hover(|s| s.bg(hover())).child(if key.is_empty() { div().t_body().text_color(subtle()).child("未设置").into_any_element() } else { keycap(key_label(&key)).into_any_element() }),
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.capturing = Some(action);
                cx.notify()
            }));
        div()
            .flex()
            .flex_col()
            .items_end()
            .gap(px(SPACE_1))
            .child(field)
            .children(conflict.map(|other| div().t_callout().text_color(danger()).child(format!("与「{other}」冲突"))))
            .into_any_element()
    }

    /// 设置窗口的一个标签页：0 通用，1 播放，2 弹幕，3 快捷键
    pub(crate) fn render_settings_tab(&self, tab: usize, cx: &mut Context<Self>) -> AnyElement {
        let st = &self.store;
        let page = div().flex().flex_col().gap(px(SPACE_6));
        match tab {
            1 => {
                // 直播设置
                let lv = &st.live;
                let buffers = [16u32, 32, 64, 128];
                let live = vec![
                    self.row("默认清晰度", None, self.choices("quality", &["最低", "中等", "最高", "记住上次"], lv.quality_level.min(3) as usize, |t, i| t.store.live.quality_level = i as u8, cx)),
                    self.row("画面尺寸", None, self.choices("fit", &["适应", "填充", "拉伸"], lv.fit.min(2) as usize, |t, i| t.store.live.fit = i as u8, cx)),
                    self.row("进入直播间自动全屏", None, self.switch("auto-full", lv.auto_fullscreen, |t, v| t.store.live.auto_fullscreen = v, cx)),
                ];
                let network = vec![
                    self.row("硬件解码", Some("关闭后改用软件解码，下次播放生效"), self.switch("hwdec", lv.hwdec, |t, v| t.store.live.hwdec = v, cx)),
                    self.row(
                        "缓冲区大小",
                        Some("播放卡顿时调高；也决定暂停或往回拖时能回看多久，约每 16 MB 回看半分钟，下次播放生效"),
                        self.choices("buffer", &["16 MB", "32 MB", "64 MB", "128 MB"], buffers.iter().position(|b| *b == lv.buffer_mb).unwrap_or(0), |t, i| t.store.live.buffer_mb = [16, 32, 64, 128][i], cx),
                    ),
                    self.row("使用 HTTPS 链接", Some("把 http 播放地址换成 https"), self.switch("https", lv.https, |t, v| t.store.live.https = v, cx)),
                ];
                page.child(self.section("播放", live)).child(self.section("网络与解码", network)).into_any_element()
            }
            2 => {
                let mut danmaku = vec![self.row("默认开启弹幕", Some("进入直播间时显示滚动弹幕"), self.switch("dm-enabled", st.danmaku.enabled, |t, v| t.store.danmaku.enabled = v, cx))];
                // 滑块行：标题在左，数值（等宽数字）和滑块靠右
                for (label, value, slider) in self.danmaku_sliders(false, cx) {
                    let hint = (label == "滚动速度").then_some("弹幕横穿画面的秒数，越小越快");
                    let control = div().flex().items_center().gap(px(SPACE_3)).child(div().w(px(56.)).t_timecode().text_right().text_color(muted()).child(value)).child(slider);
                    danmaku.push(self.row(label, hint, control));
                }
                page.child(self.section("弹幕", danmaku)).into_any_element()
            }
            3 => {
                let mut keys: Vec<AnyElement> = KEY_ACTIONS.iter().map(|&(action, label, _)| self.row(label, None, self.key_field(action, cx))).collect();
                keys.push(self.row(
                    "恢复默认快捷键",
                    Some("只在直播间里生效；点按键框后按下新的按键，Esc 取消，⌫ 清除"),
                    button("keys-reset", Btn::Normal, Some("rotate-cw"), "恢复默认").on_click(cx.listener(|this, _, _, cx| {
                        this.store.keys = Store::default().keys;
                        this.save_and_notify(cx)
                    })),
                ));
                page.child(self.section("直播间快捷键", keys)).into_any_element()
            }
            _ => {
                // 账号
                let bili_control = match (&self.login, st.bili.cookie.is_empty()) {
                    (Some(_), _) => button("bili-cancel", Btn::Ghost, None, "取消").on_click(cx.listener(|this, _, _, cx| {
                        this.login = None;
                        cx.notify()
                    })),
                    (None, true) => button("bili-login", Btn::Normal, None, "扫码登录").on_click(cx.listener(|this, _, _, cx| this.start_bili_login(cx))),
                    (None, false) => button("bili-logout", Btn::Normal, None, "退出登录").on_click(cx.listener(|this, _, _, cx| this.logout_bili(cx))),
                };
                let bili_sub = if st.bili.cookie.is_empty() { "未登录，登录后可以看更高清晰度" } else { "已登录" };
                let mut account = vec![self.row("哔哩哔哩", Some(bili_sub), div().flex().items_center().gap_3().children((!st.bili.name.is_empty()).then(|| div().t_body().text_color(accent()).child(st.bili.name.clone()))).child(bili_control))];
                if let Some(login) = &self.login {
                    // 二维码画成小方格；白底留边方便识别
                    let cell = 4.;
                    let qr = div().p(px(SPACE_3)).bg(rgb(0xffffff)).rounded(px(RADIUS_MD)).flex().flex_col().children(login.qr.chunks(login.width.max(1)).map(|row| div().flex().children(row.iter().map(|&dark| div().size(px(cell)).bg(rgb(if dark { 0x000000 } else { 0xffffff }))))));
                    account.push(div().p(px(SPACE_4)).flex().items_center().gap(px(SPACE_4)).children((!login.qr.is_empty()).then_some(qr)).child(div().t_body().text_color(muted()).child(login.status)).into_any_element());
                }
                account.push(self.row("斗鱼", Some("暂不支持登录，不登录也能正常观看"), div()));
                account.push(self.row("虎牙", Some("不需要登录"), div()));

                let theme_idx = match st.theme {
                    Theme::System => 0,
                    Theme::Light => 1,
                    Theme::Dark => 2,
                };
                let appearance = vec![self.row("外观", None, self.choices("theme", &["跟随系统", "浅色", "深色"], theme_idx, |t, i| t.store.theme = [Theme::System, Theme::Light, Theme::Dark][i], cx))];

                let home_items = st.home_sort.iter().filter_map(|id| HOME_PAGES.iter().find(|(p, _)| p == id).map(|(_, n)| n.to_string())).collect();
                let site_items = self.sites.iter().map(|s| s.name().to_string()).collect();
                let home = vec![
                    self.row("页面顺序", Some("排第一的是启动页"), self.sort_list("home", home_items, Some("启动页"), |t, i, up, cx| t.move_home(i, up, cx), cx)),
                    self.row("平台顺序", None, self.sort_list("sites", site_items, None, |t, i, up, cx| t.move_site(i, up, cx), cx)),
                ];

                let fs = st.follow;
                let intervals = [5u32, 10, 15, 30, 60];
                let threads = [1u32, 2, 4, 8];
                let follow = vec![
                    self.row("自动更新开播状态", None, self.switch("follow-auto", fs.auto_refresh, |t, v| t.store.follow.auto_refresh = v, cx)),
                    self.row("更新间隔", None, self.popup("follow-interval", &["5 分钟", "10 分钟", "15 分钟", "30 分钟", "60 分钟"], intervals.iter().position(|v| *v == fs.interval_min).unwrap_or(1), |t, i| t.store.follow.interval_min = [5, 10, 15, 30, 60][i], cx)),
                    self.row("同时查询的房间数", None, self.choices("follow-threads", &["1", "2", "4", "8"], threads.iter().position(|v| *v == fs.threads).unwrap_or(2), |t, i| t.store.follow.threads = [1, 2, 4, 8][i], cx)),
                ];

                let ae = st.auto_exit;
                let minutes = [15u32, 30, 60, 90, 120];
                let auto_exit = vec![
                    self.row("定时关闭", Some("从进入直播间开始倒计时，到时退出 Livo"), self.switch("exit-on", ae.enabled, |t, v| t.store.auto_exit.enabled = v, cx)),
                    self.row("关闭时间", None, self.popup("exit-min", &["15 分钟", "30 分钟", "1 小时", "1.5 小时", "2 小时"], minutes.iter().position(|v| *v == ae.minutes).unwrap_or(2), |t, i| t.store.auto_exit.minutes = [15, 30, 60, 90, 120][i], cx)),
                ];

                let history = vec![self.row(
                    "观看记录",
                    Some(if st.history.is_empty() { "还没有观看记录" } else { "清空后不能恢复，关注的主播不受影响" }),
                    button("settings-clear-history", Btn::Normal, Some("trash-2"), "清空记录…").on_click(cx.listener(|this, _, window, cx| this.confirm_clear_history(window, cx))),
                )];

                page.child(self.section("账号", account))
                    .child(self.section("外观", appearance))
                    .child(self.section("主页", home))
                    .child(self.section("关注", follow))
                    .child(self.section("定时关闭", auto_exit))
                    .child(self.section("隐私", history))
                    .into_any_element()
            }
        }
    }

    pub(crate) fn render_about(&self, cx: &mut Context<Self>) -> impl IntoElement {
        use crate::about::{can_install, Update, REPO, VERSION};
        let (status, color) = match &self.update {
            Update::Idle => (String::new(), muted()),
            Update::Checking => ("正在检查…".to_string(), muted()),
            Update::Latest => ("已是最新版本".to_string(), muted()),
            Update::Available { version, .. } => (format!("发现新版本 {version}"), accent()),
            Update::Downloading { got, total, .. } if *total > 0 => (format!("正在下载 {}%", got * 100 / total), accent()),
            Update::Downloading { got, .. } => (format!("正在下载 {:.1}MB", *got as f64 / 1048576.), accent()),
            Update::Ready { version, .. } => (format!("{version} 已就绪，退出时自动安装"), accent()),
            Update::Failed(e) => (e.clone(), c(pal().danger)),
        };
        let mut update = vec![self.row(
            "检查更新",
            None,
            div().flex().items_center().gap(px(SPACE_3)).child(div().t_body().text_color(color).child(status)).child(button("check-update", Btn::Normal, None, "检查更新").on_click(cx.listener(|this, _, _, cx| this.check_update(false, cx)))),
        )];
        if let Update::Available { version, desc } | Update::Downloading { version, desc, .. } | Update::Ready { version, desc } = &self.update {
            let url = format!("{REPO}/releases/tag/v{version}");
            // 能自动装就下载/重启，开发构建和没出包的平台只能去网页下载
            let action = match &self.update {
                Update::Available { .. } if can_install(version) => Some(button("download-update", Btn::Primary, None, "下载并安装").on_click(cx.listener(|this, _, _, cx| this.download_update(cx)))),
                Update::Available { .. } => Some(button("open-release", Btn::Primary, Some("external-link"), "前往下载").on_click(cx.listener(move |_, _, _, cx| cx.open_url(&url)))),
                Update::Ready { .. } => Some(button("install-update", Btn::Primary, None, "立即重启并更新").on_click(cx.listener(|this, _, _, cx| this.install_update(true, cx)))),
                _ => None,
            };
            update.push(
                div()
                    .p(px(SPACE_4))
                    .flex()
                    .flex_col()
                    .items_start()
                    .gap(px(SPACE_3))
                    .child(div().t_callout().text_color(muted()).child(desc.clone()))
                    .children(action)
                    .into_any_element(),
            );
        }
        update.push(self.row("自动更新", Some("启动时和每 6 小时检查新版本，后台下载并校验签名，退出时自动安装"), self.switch("auto-update", self.store.auto_update, |t, v| t.store.auto_update = v, cx)));
        update.push(self.row("接收测试版", Some("也更新到预发行版本"), self.switch("update-beta", self.store.update_beta, |t, v| t.store.update_beta = v, cx)));
        div()
            .max_w(px(640.))
            .flex()
            .flex_col()
            .gap(px(SPACE_6))
            // 关于页用组合标识（字标已经写着 LIVO，不再另放标题）；四周留白 ≥ 0.5H
            .child(
                div()
                    .py(px(SPACE_4))
                    .flex()
                    .flex_col()
                    .items_start()
                    .gap(px(SPACE_6))
                    .child(img(gpui::ImageSource::Resource(gpui::Resource::Embedded(if LIGHT.load(Ordering::Relaxed) { "about-lockup.svg" } else { "about-lockup-reverse.svg" }.into()))).w(px(160.)).h(px(65.28)).flex_none())
                    .child(div().flex().flex_col().gap(px(SPACE_1)).child(div().t_body().text_color(muted()).child(format!("版本 {VERSION}"))).child(div().t_callout().text_color(subtle()).child("哔哩哔哩、斗鱼、虎牙的直播放进同一个窗口"))),
            )
            .child(self.section("更新", update))
            .child(self.section(
                "项目",
                vec![self.row("GitHub", Some(REPO), button("open-repo", Btn::Normal, Some("external-link"), "在浏览器中打开").on_click(cx.listener(|_, _, _, cx| cx.open_url(REPO))))],
            ))
    }

    pub(crate) fn render_history(&self, width: f32, cx: &mut Context<Self>) -> impl IntoElement {
        if self.store.history.is_empty() {
            return empty_state("history", "还没有观看记录", "看过的直播间会出现在这里", None).into_any_element();
        }
        let (_, card_w) = grid_cols(width);
        let cards = self.store.history.iter().enumerate().map(|(i, h)| {
            let (site_id, room_id) = (self.site_index(&h.site).map_or("", |i| self.sites[i].id()), h.room_id.clone());
            let follow = Follow { site: h.site.clone(), room_id: h.room_id.clone(), user_name: h.user_name.clone(), face: h.face.clone(), title: h.title.clone(), cover: h.cover.clone(), ..Default::default() };
            let key = (h.site.clone(), h.room_id.clone());
            // 删除按钮浮在封面右上角，悬停卡片时出现；能撤销的小操作，不弹确认
            let delete = div()
                .id(SharedString::from(format!("history-del-{i}")))
                .absolute()
                .top(px(SPACE_2))
                .right(px(SPACE_2))
                .size(px(theme::CONTROL_SM))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(c(theme::DARK.scrim))
                .cursor_pointer()
                .opacity(0.)
                .group_hover("card", |s| s.opacity(1.))
                .tooltip(tip("从记录中移除", None))
                .child(icon("x", 14., c(theme::DARK.on_media)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.store.history.remove(i);
                    this.save_and_notify(cx);
                    cx.stop_propagation()
                }));
            div().relative().child(
                self.card(SharedString::from(format!("history-{}-{}", h.site, h.room_id)), card_w, &h.cover, &h.title, &format!("{} · {}", h.user_name, time_ago(h.time)), &h.face, &h.site, CardLive::Unknown)
                    .on_click(cx.listener(move |this, _, _, cx| this.nav(cx, |t, cx| t.open_on(site_id, room_id.clone(), cx))))
                    .on_mouse_down(MouseButton::Right, cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                        let key = key.clone();
                        let remove = MenuEntry::item(
                            "从历史中移除",
                            Some("trash-2"),
                            Rc::new(move |t: &mut Livo, _: &mut Window, cx: &mut Context<Livo>| {
                                t.store.history.retain(|h| (&h.site, &h.room_id) != (&key.0, &key.1));
                                t.save_and_notify(cx)
                            }) as Act,
                        )
                        .danger();
                        this.open_ctx(ev.position, this.room_menu(follow.clone(), true, vec![remove]), false, cx)
                    }))
                    .child(delete),
            )
        });
        div().flex().flex_wrap().gap(px(GAP)).children(cards).into_any_element()
    }
}

/// 设置窗口的标签：(名字, 20px 图标)
const SETTINGS_TABS: [(&str, &str); 4] = [("通用", "settings"), ("播放", "play"), ("弹幕", "message-square-text"), ("快捷键", "keyboard")];

/// 设置窗口（⌘,，Apple 的设置窗口惯例）：600 宽、不可调整大小；工具栏标签切换分类，窗口标题随标签变化；
/// 修改立即生效，没有「保存」。内容由 Livo 渲染，Livo 一变这里就重绘
pub(crate) struct SettingsWindow {
    livo: Entity<Livo>,
    tab: usize,
    focus: FocusHandle,
}

impl Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.livo.read(cx).store.theme;
        let light = match theme {
            Theme::Light => true,
            Theme::Dark => false,
            Theme::System => matches!(window.appearance(), gpui::WindowAppearance::Light | gpui::WindowAppearance::VibrantLight),
        };
        LIGHT.store(light, Ordering::Relaxed);
        let (title, _) = SETTINGS_TABS[self.tab];
        window.set_window_title(title);
        let tab = self.tab;
        let body = self.livo.update(cx, |l, cx| l.render_settings_tab(tab, cx));
        let menu = self.livo.update(cx, |l, cx| l.render_ctx_menu(true, cx));
        let tabs = SETTINGS_TABS.iter().enumerate().map(|(i, (name, glyph))| {
            let on = i == self.tab;
            div()
                .id(("settings-tab", i))
                .w(px(72.))
                .h(px(48.))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(2.))
                .rounded(px(RADIUS_SM))
                .cursor_pointer()
                .focusable_ring()
                .when(on, |d| d.bg(hover()))
                .when(!on, |d| d.hover(|s| s.bg(hover())))
                .child(icon(glyph, 20., if on { accent() } else { muted() }))
                .child(div().t_caption().text_color(if on { accent() } else { muted() }).child(*name))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.tab = i;
                    cx.notify()
                }))
        });
        let livo = self.livo.clone();
        div()
            .id("settings-window")
            .track_focus(&self.focus)
            .key_context("Settings")
            .on_key_down(cx.listener(move |_, ev: &KeyDownEvent, _, cx| {
                livo.update(cx, |l, cx| {
                    if ev.keystroke.key == "escape" && l.ctx_menu.is_some() {
                        l.ctx_menu = None;
                        return cx.notify();
                    }
                    l.capture_key(&ev.keystroke.key, cx);
                })
            }))
            .on_action(|_: &menus::CloseWindow, window, _| window.remove_window())
            .on_action(|_: &menus::FocusNext, window, _| window.focus_next())
            .on_action(|_: &menus::FocusPrev, window, _| window.focus_prev())
            .size_full()
            .flex()
            .flex_col()
            .bg(bg())
            .text_color(text())
            .font_family(theme::FONT_UI)
            .t_body()
            // 标题栏与工具栏合一：红绿灯那一行放窗口标题，下面一排标签
            .child(
                div()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .items_center()
                    .border_b_1()
                    .border_color(border())
                    .bg(sidebar())
                    .child(div().h(px(28.)).flex().items_center().t_headline().child(title))
                    .child(div().pb(px(SPACE_2)).flex().gap(px(SPACE_1)).children(tabs)),
            )
            .child(div().id("settings-body").flex_1().overflow_y_scroll().p(px(PAD)).child(body))
            .children(menu)
    }
}

impl Livo {
    /// 打开设置窗口；已经开着就拿到前面
    pub(crate) fn open_settings(&mut self, cx: &mut Context<Self>) {
        if let Some(h) = self.settings_win {
            if h.update(cx, |_, window, _| window.activate_window()).is_ok() {
                return;
            }
        }
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(600.), px(640.)), cx))),
            titlebar: Some(TitlebarOptions { title: Some("通用".into()), appears_transparent: true, traffic_light_position: Some(point(px(12.), px(8.))) }),
            is_resizable: false,
            is_minimizable: true,
            ..Default::default()
        };
        let livo = cx.entity();
        match cx.open_window(options, |window, cx| {
            cx.new(|cx| {
                cx.observe(&livo, |_, _, cx| cx.notify()).detach();
                // 关掉设置窗口时 Livo 忘掉它
                cx.on_release(|this: &mut SettingsWindow, cx| {
                    this.livo.update(cx, |l, _| {
                        l.settings_win = None;
                        l.capturing = None;
                        l.ctx_menu = None;
                    })
                })
                .detach();
                let focus = cx.focus_handle();
                window.focus(&focus);
                SettingsWindow { livo, tab: 0, focus }
            })
        }) {
            Ok(h) => self.settings_win = Some(h),
            Err(e) => self.error = Some(format!("打不开设置窗口：{e}")),
        }
    }

    /// 正在录入快捷键时，这一下按键就是新绑定：Esc 取消，⌫ 清除。返回 true 表示按键已被录入用掉
    pub(crate) fn capture_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        let Some(action) = self.capturing.take() else { return false };
        match key {
            "escape" => {}
            "backspace" => {
                self.store.keys.insert(action.to_string(), String::new());
                self.store.save();
            }
            _ => {
                self.store.keys.insert(action.to_string(), key.to_string());
                self.store.save();
            }
        }
        cx.notify();
        true
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
