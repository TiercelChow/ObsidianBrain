import { createApp } from 'vue'
import { createPinia } from 'pinia'
import {
  ElButton,
  ElContainer,
  ElDatePicker,
  ElDescriptions,
  ElDescriptionsItem,
  ElDialog,
  ElEmpty,
  ElForm,
  ElFormItem,
  ElIcon,
  ElInput,
  ElInputNumber,
  ElMain,
  ElOption,
  ElSelect,
  ElSlider,
  ElSwitch,
  ElTag,
} from 'element-plus'
import 'element-plus/dist/index.css'
import 'element-plus/theme-chalk/dark/css-vars.css'
import './styles/motion.css'
import App from './App.vue'
import router from './router'

// Apply saved theme BEFORE app mount to prevent flash of wrong theme
const savedTheme = localStorage.getItem('theme') as 'light' | 'dark' | 'eye-care' | null
document.documentElement.setAttribute('data-theme', savedTheme || 'light')

const app = createApp(App)

const elementComponents = {
  ElButton,
  ElContainer,
  ElDatePicker,
  ElDescriptions,
  ElDescriptionsItem,
  ElDialog,
  ElEmpty,
  ElForm,
  ElFormItem,
  ElIcon,
  ElInput,
  ElInputNumber,
  ElMain,
  ElOption,
  ElSelect,
  ElSlider,
  ElSwitch,
  ElTag,
}
for (const [name, component] of Object.entries(elementComponents)) {
  app.component(name, component)
}

app.use(createPinia())
app.use(router)

app.mount('#app')
