# Galroon Frontend - Tailwind CSS 开发规范

## 📋 概述

本项目使用 **Tailwind CSS v4.1.18** 进行样式开发。所有样式必须使用 Tailwind 类名，禁止使用 inline styles（除了动态计算的值）。

---

## 🎨 设计系统配置

### 主题定义 (globals.css)

```css
@theme {
  /* 品牌颜色 */
  --color-background: #0B0C0F;
  --color-surface: #121212;
  --color-surface-elevated: #1A1A1A;

  /* 文字颜色 */
  --color-text-primary: #FFFFFF;
  --color-text-secondary: #B3B3B3;
  --color-text-tertiary: #6B6B6B;

  /* 强调色 */
  --color-accent-blue: #7BA8C7;
  --color-accent-gold: #FF9100;

  /* 边框颜色（带透明度） */
  --color-border-subtle: rgba(255, 255, 255, 0.05);
  --color-border-medium: rgba(255, 255, 255, 0.1);
  --color-border-strong: rgba(255, 255, 255, 0.15);

  /* 圆角 */
  --radius-sm: 4px;
  --radius-md: 8px;
  --radius-lg: 12px;
  --radius-xl: 16px;
  --radius-2xl: 24px;
  --radius-pill: 9999px;
}
```

---

## ✅ 正确用法

### 1. 使用任意值语法

对于设计系统中的颜色值，使用 Tailwind 的任意值语法：

```tsx
{/* ✅ 正确 - 使用任意值语法 */}
<div className="border-[rgba(255,255,255,0.05)]" />
<div className="bg-[#121212]" />
<div className="w-[280px]" />

{/* ❌ 错误 - 使用 inline style */}
<div style={{ borderColor: 'rgba(255,255,255,0.05)' }} />
<div style={{ backgroundColor: '#121212' }} />
<div style={{ width: '280px' }} />
```

### 2. 透明度颜色

**重要**: Tailwind v4 不支持 `border-white/5` 这种旧语法！

```tsx
{/* ✅ 正确 - Tailwind v4 语法 */}
<div className="border-[rgba(255,255,255,0.05)]" />
<div className="bg-white/5" /> {/* opacity 值仍可用于背景色 */}

{/* ❌ 错误 - 旧版本透明度语法（会被渲染成纯白色） */}
<div className="border-white/5" />
<div className="border-white/10" />
```

### 3. 边框颜色映射

| 用途 | Tailwind 类 | 颜色值 |
|------|------------|--------|
| 微妙边框 | `border-[rgba(255,255,255,0.05)]` | 5% 白色 |
| 中等边框 | `border-[rgba(255,255,255,0.1)]` | 10% 白色 |
| 强调边框 | `border-[rgba(255,255,255,0.15)]` | 15% 白色 |

---

## ⚠️ 允许使用 inline style 的情况

只有以下情况可以（且必须）使用 inline style：

### 1. 动态计算的值

```tsx
{/* ✅ 正确 - 动态背景色 */}
<div
  className="w-10 h-10 rounded-full"
  style={{
    backgroundColor: user.avatarColor || '#FF9100',
    color: user.avatarColor ? '#FFFFFF' : '#000000'
  }}
/>
```

### 2. CSS 变量（如果 Tailwind 无法访问）

```tsx
{/* ✅ 正确 - CSS 变量 */}
<div style={{ borderColor: 'var(--border-subtle)' }} />
```

---

## 🚫 常见错误

### 错误 1: 使用旧版透明度语法

```tsx
{/* ❌ 错误 - 会在 Tailwind v4 中被渲染成纯白色 */}
<div className="border-white/5" />

/* ✅ 正确 */
<div className="border-[rgba(255,255,255,0.05)]" />
```

### 错误 2: 混用 style 和 className

```tsx
{/* ❌ 错误 - 两个 style 属性 */}
<button
  className="w-[280px]"
  style={{ borderColor: 'var(--border-subtle)' }}
  style={{ width: '280px' }}  {/* 重复! */}
/>

/* ✅ 正确 - 合并到一个 style */
<button
  className="w-[280px]"
  style={{ borderColor: 'var(--border-subtle)' }}
/>

/* ✅ 更好 - 全部使用 Tailwind */
<button className="w-[280px] border-[rgba(255,255,255,0.05)]" />
```

### 错误 3: 将 className 放在模板字符串中

```tsx
{/* ❌ 错误 */}
<div
  className={`bg-white ${someCondition}`}
  className={`w-full`}  {/* 重复! */}
/>

/* ✅ 正确 - 所有类在一个 className 中 */
<div className={`bg-white w-full ${someCondition}`} />
```

---

## 📐 常用模式

### 卡片样式

```tsx
<div className="bg-[#121212] border border-[rgba(255,255,255,0.05)] rounded-2xl">
  {/* 内容 */}
</div>
```

### 按钮样式

```tsx
<button className="px-4 py-2 bg-white/5 hover:bg-white/10 border border-[rgba(255,255,255,0.1)] rounded-lg text-white transition-colors cursor-pointer">
  按钮文字
</button>
```

### 容器最大宽度

```tsx
<div className="max-w-[1400px] mx-auto">
  {/* 内容 */}
</div>
```

### 固定宽度/高度

```tsx
<div className="w-[280px] h-[200px]">
  {/* 内容 */}
</div>
```

---

## 🔍 检查清单

在提交代码前，确保：

- [ ] 没有使用 `border-white/5`、`border-white/10` 等旧版透明度语法
- [ ] 所有固定值都使用 Tailwind 任意值语法：`bg-[#121212]`、`w-[280px]`
- [ ] 没有重复的 `style` 或 `className` 属性
- [ ] 动态值使用 inline style，其他都用 className
- [ ] 所有交互元素有 `cursor-pointer` 类
- [ ] 过渡动画使用 `transition-*` 类

---

## 📚 参考资料

- [Tailwind CSS v4 文档](https://tailwindcss.com/docs/v4-beta)
- [任意值语法](https://tailwindcss.com/docs/adding-custom-styles#using-arbitrary-values)
- [项目设计规范](./CLAUDE.md)

---

**最后更新**: 2025-01-07
**维护者**: Claude (Sonnet 4.5)
**版本**: 1.0.0
