import TaskBar from "./TaskBar";
import { Outlet } from "react-router-dom";

export default function Layout() {
  return (
    <>
      <TaskBar/>
      <Outlet/>
    </>
  )
}