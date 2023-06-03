import { useEffect, useState } from 'react'

type Maybe<T> = T | null

export function useFutureResult<T> (promise: Promise<T>): Maybe<T> {
  const [result, setResult] = useState<Maybe<T>>(null)
  useEffect(() => {
    promise.then(value => { setResult(value) })
    // Todo: kill off after component unmounts.
  }, [])
  return result
}
