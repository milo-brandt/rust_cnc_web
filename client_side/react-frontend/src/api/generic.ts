import { useEffect, useMemo, useState } from 'react'
import { HOST } from './constants'
import { type Maybe } from '../util/types'
import { cncAxios } from './cncAxios'

export function useWebSocket<T> (path: string, reader: (value: any) => Maybe<T>): Maybe<T> {
  const [result, setResult] = useState<Maybe<T>>(null)
  useEffect(() => {
    const ws = new WebSocket(`ws://${HOST}${path}`)
    ws.onmessage = (event) => {
      setResult(reader(JSON.parse(event.data as string)))
    }
    return () => {
      ws.close()
    }
  }, [])
  return result
}

interface PromiseLoading {
  status: "loading";
}
interface PromiseResolved<T> {
  status: "resolved";
  data: T;
}
interface PromiseRejected {
  status: "rejected";
  error: any;
}

export type PromiseResult<T> = PromiseLoading | PromiseResolved<T> | PromiseRejected;
export interface ReloadablePromiseResult<T> {
  result: PromiseResult<T>,
  reload: () => void,
}

export function useMaybeCancellablePromise<T>(promise: Maybe<(signal: AbortSignal) => Promise<T>>): Maybe<PromiseResult<T>> {
  const [result, setResult] = useState<Maybe<PromiseResult<T>>>(promise === null ? null : {status: "loading"});
  useEffect(() => {
    if(promise === null) {
      setResult(null);
    } else {
      setResult({status: "loading"});
      let isActive = true;
      let abortController = new AbortController();
      let abortSignal = abortController.signal;
      promise(abortSignal).then(data => {
        if(isActive) {
          setResult({status: "resolved", data})
        }
      }).catch(error => {
        if(isActive) {
          setResult({status: "rejected", error})
        }
      });
      return () => {
        isActive = false;
        abortController.abort();
      }
    }
  }, [promise]);
  return result;
}
export function useCancellablePromise<T>(promise: (signal: AbortSignal) => Promise<T>): PromiseResult<T> {
  let result = useMaybeCancellablePromise(promise);
  return result!;
}

export function useGet<T>(path: string): ReloadablePromiseResult<T> {
  const [generation, setGeneration] = useState(0);
  return {
    result: useCancellablePromise(
      useMemo(
        () => async (signal: AbortSignal) => {
          // await new Promise(resolve => setTimeout(resolve, 500));
          let result = await cncAxios.get<T>(path, { signal });
          return result.data;
        },
        [path, generation]
      )
    ),
    reload: () => {
      setGeneration(generation + 1)
    }
  }
}