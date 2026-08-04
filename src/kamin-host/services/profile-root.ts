// Общий детект «профильного» корня (диск целиком, %USERPROFILE%,
// C:\Users\<name>) — раньше вотчер и файловый индекс держали по СВОЕЙ
// версии с разными правилами и молча расходились (ревью): вотчер знал про
// голый диск и env, индекс — только про C:/Users/<name>.

/** Корень размером с профиль пользователя — не проект. */
export function isProfileSizedRoot(root: string): boolean {
  const norm = root.replace(/\//g, "\\").replace(/\\+$/, "")
  if (/^[a-z]:$/i.test(norm)) return true
  const home = (process.env.USERPROFILE ?? process.env.HOME ?? "").replace(/\\+$/, "")
  if (home !== "" && norm.toLowerCase() === home.toLowerCase()) return true
  return /^[a-z]:\\Users\\[^\\]+$/i.test(norm)
}
