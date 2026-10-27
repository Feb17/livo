use livo_core::{danmaku, Site};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    for b in Site::all() {
        println!("== {}", b.name());
        let cats = b.categories().await?;
        println!("分类 {} 个，首个 {} 含 {} 子类", cats.len(), cats[0].name, cats[0].children.len());
        let cr = b.category_rooms(&cats[0].children[0], 1).await?;
        println!("分类房间 {} 个", cr.items.len());
        let rec = b.recommend_rooms(1).await?;
        println!("推荐 {} 个房间", rec.items.len());
        let room = &rec.items[0];
        println!("房间 {} {}", room.room_id, room.title);
        let d = b.room_detail(&room.room_id).await?;
        println!("详情 {} 主播 {} 直播中 {}", d.title, d.user_name, d.live);
        let qs = b.play_qualities(&d.room_id).await?;
        println!("清晰度 {:?}", qs.iter().map(|q| &q.name).collect::<Vec<_>>());
        let pu = b.play_urls(&d.room_id, qs[0].qn).await?;
        println!("播放地址 {} 条，首条 {:.80}", pu.urls.len(), pu.urls[0]);
        let s = b.search_rooms("英雄联盟", 1).await?;
        println!("搜索 {} 条", s.items.len());
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(danmaku::run(d.danmaku.clone(), tx));
        for _ in 0..3 {
            match tokio::time::timeout(std::time::Duration::from_secs(15), rx.recv()).await {
                Ok(Some(m)) => println!("弹幕 {m:?}"),
                _ => break,
            }
        }
    }
    Ok(())
}
