sqlxのqueryファミリーは、できるだけ多くのことをランタイムやテストではなくコンパイルで検証したいため関数のものではなくマクロのものを使う (e.g. `sqlx::query("...").bind(value)`ではなく`sqlx::query!("...", value)`)
