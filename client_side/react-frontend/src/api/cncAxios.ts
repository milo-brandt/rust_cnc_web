import axios from 'axios'
import { HOST } from './constants'

export const cncAxios = axios.create({
  baseURL: `http://${HOST}`,
  timeout: 1000
})
