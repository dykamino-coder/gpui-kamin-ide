declare module '*.module.css' {
  const classes: Record<string, string>
  export default classes
}

declare module '*.png' {
  const url: string
  export default url
}

declare module '*.jpg' {
  const url: string
  export default url
}

declare module '*.svg' {
  const url: string
  export default url
}

declare module '*.svg?raw' {
  const source: string
  export default source
}
