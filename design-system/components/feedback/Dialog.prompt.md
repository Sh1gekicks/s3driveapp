Modal sheet over the window (overlay is `position:absolute`, so place inside a positioned container / AppWindow). Primary button is rightmost.
```jsx
<Dialog icon="trash-2" tone="destructive" title="2 項目を削除しますか？" description="この操作は取り消せません。"
  footer={<><Button variant="outline">キャンセル</Button><Button variant="destructive">削除</Button></>} />
```
