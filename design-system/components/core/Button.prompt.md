Push button (shadcn variants, macOS sizing). One `default` per view — the primary action (アップロード, 保存). Others use outline/ghost.
```jsx
<Button icon="upload">アップロード</Button>
<Button variant="outline">キャンセル</Button>
<Button variant="destructive" icon="trash-2">削除</Button>
```
Sizes: sm (toolbars in dense panels), md (default), lg (sign-in screen).
