/** 常用正则（静态示例） */
export const PRESETS: { key: string; pattern: string }[] = [
  { key: 'email', pattern: String.raw`[\w.+-]+@[\w-]+(\.[\w-]+)+` },
  { key: 'url', pattern: String.raw`https?://[^\s/$.?#].[^\s]*` },
  { key: 'ipv4', pattern: String.raw`\b(?:(?:25[0-5]|2[0-4]\d|1?\d?\d)\.){3}(?:25[0-5]|2[0-4]\d|1?\d?\d)\b` },
  { key: 'phoneCn', pattern: String.raw`\b1[3-9]\d{9}\b` },
  { key: 'idCardCn', pattern: String.raw`\b\d{17}[\dXx]\b` },
  { key: 'date', pattern: String.raw`(?P<year>\d{4})-(?P<month>0[1-9]|1[0-2])-(?P<day>0[1-9]|[12]\d|3[01])` },
  { key: 'time', pattern: String.raw`\b(?:[01]\d|2[0-3]):[0-5]\d(?::[0-5]\d)?\b` },
  { key: 'hexColor', pattern: String.raw`#(?:[0-9a-fA-F]{3}){1,2}\b` },
  { key: 'chinese', pattern: String.raw`[\p{Han}]+` },
  { key: 'number', pattern: String.raw`-?\d+(?:\.\d+)?` },
]

export const SAMPLE_TEXT = `联系人：张三 <zhangsan@example.com>，电话 13812345678
备用邮箱 ops+alerts@tool-forge.dev，官网 https://toolforge.dev/docs?lang=zh
服务器 192.168.1.20 与 10.0.0.255 在 2024-02-29 08:30:00 完成部署
主题色 #1c9a4f 与 #fff，版本 0.1.0，耗时 12.5 秒`
