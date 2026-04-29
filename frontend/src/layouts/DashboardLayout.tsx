import React from 'react'
import { Outlet } from 'react-router-dom'
import Sidebar from '@/components/Sidebar'
import { Toaster } from 'sonner'
import { GlobalErrorBoundary } from '@/components/GlobalErrorBoundary'

export const DashboardLayout: React.FC = () => {
  return (
    <div className="flex min-h-screen bg-background text-foreground">
      {/* Sidebar - we will pass navigation logic later */}
      <Sidebar />
      
      <main className="flex-1 flex flex-col min-w-0 overflow-hidden">
        {/* Header can be added here if needed */}
        <div className="flex-1 overflow-y-auto px-6 py-8">
          <div className="max-w-7xl mx-auto">
            <GlobalErrorBoundary>
              <Outlet />
            </GlobalErrorBoundary>
          </div>
        </div>
      </main>

      <Toaster theme="dark" position="top-right" closeButton />
    </div>
  )
}
