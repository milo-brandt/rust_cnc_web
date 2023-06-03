import { cncAxios } from './cncAxios'

export async function pause () {
  return await cncAxios.post('/command/pause')
}
export async function resume () {
  return await cncAxios.post('/command/resume')
}
export async function stop () {
  return await cncAxios.post('/command/stop')
}
export async function reset () {
  return await cncAxios.post('/command/reset')
}
export async function shutdown () {
  return await cncAxios.post('/shutdown')
}
