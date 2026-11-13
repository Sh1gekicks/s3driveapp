-- 版 2: 検索インデックスの更新を軽くする（06 §4.3）

-- 再走査では同じ名前の行を上書きする。name_norm が変わらない更新では FTS を書き直さない
-- （UPDATE OF は、値が同じでも SET 句に列があれば発火するため、WHEN で絞る）
DROP TRIGGER objects_au;
CREATE TRIGGER objects_au AFTER UPDATE OF name_norm ON objects
WHEN old.name_norm <> new.name_norm
BEGIN
  INSERT INTO objects_fts (objects_fts, rowid, name_norm) VALUES ('delete', old.id, old.name_norm);
  INSERT INTO objects_fts (rowid, name_norm) VALUES (new.id, new.name_norm);
END;

-- フォルダマーカーだけの索引。一覧のフォルダの更新日（04 §3.2）を、フォルダ直下の全行を読まずに引く
CREATE INDEX objects_markers ON objects (connection_id, parent) WHERE is_marker = 1;
