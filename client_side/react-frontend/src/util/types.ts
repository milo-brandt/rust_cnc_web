export type Maybe<T> = T | null

export function mapMaybe<T, U> (map: (value: T) => U, value: Maybe<T>): Maybe<U> {
  if (value === null) {
    return null
  } else {
    return map(value)
  }
}
