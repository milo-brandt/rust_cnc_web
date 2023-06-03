import TaskBar from "./TaskBar";
import { Outlet } from "react-router-dom";

export function Layout() {
  return (
    <>
      <TaskBar/>
      <Outlet/>
    </>
  )
}