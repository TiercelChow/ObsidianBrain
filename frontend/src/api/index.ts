import axios from 'axios'

const api = axios.create({
  baseURL: '/v1',
  timeout: 10000,
  headers: {
    'Content-Type': 'application/json',
  },
})

// 请求拦截器
api.interceptors.request.use(
  (config) => config,
  (error) => {
    console.error('请求错误:', error)
    return Promise.reject(error)
  }
)

// 响应拦截器
api.interceptors.response.use(
  (response) => response.data,
  (error) => {
    console.error('响应错误:', error.response?.data || error.message)
    return Promise.reject(error)
  }
)

export default api

export function getHealth() {
  return api.get('/health')
}
