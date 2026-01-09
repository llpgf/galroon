import { X, Plus, Trash2, Lock, Edit2, Music, FileText, User } from 'lucide-react';
import { useState } from 'react';

interface MetadataEditorProps {
  isOpen: boolean;
  onClose: () => void;
  game: {
    title: string;
    originalTitle?: string;
    developer?: string;
    releaseDate?: string;
    description?: string;
    tags?: string[];
    rating?: number;
    playStatus?: string;
    staff?: { role: string; name: string }[];
  };
  onSave: (data: any) => void;
}

export function MetadataEditor({ isOpen, onClose, game, onSave }: MetadataEditorProps) {
  const [activeTab, setActiveTab] = useState('basic');
  const [formData, setFormData] = useState(game);
  const [newTag, setNewTag] = useState('');
  const [newStaff, setNewStaff] = useState({ role: '', name: '' });

  if (!isOpen) return null;

  const handleAddTag = () => {
    if (newTag.trim()) {
      setFormData({
        ...formData,
        tags: [...(formData.tags || []), newTag.trim()]
      });
      setNewTag('');
    }
  };

  const handleRemoveTag = (index: number) => {
    setFormData({
      ...formData,
      tags: formData.tags?.filter((_, i) => i !== index) || []
    });
  };

  const handleAddStaff = () => {
    if (newStaff.role && newStaff.name) {
      setFormData({
        ...formData,
        staff: [...(formData.staff || []), { ...newStaff }]
      });
      setNewStaff({ role: '', name: '' });
    }
  };

  const handleRemoveStaff = (index: number) => {
    setFormData({
      ...formData,
      staff: formData.staff?.filter((_, i) => i !== index) || []
    });
  };

  const handleSave = () => {
    onSave(formData);
    onClose();
  };

  const handleRevert = () => {
    setFormData(game);
  };

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-8">
      <div className="bg-[#282828] rounded-lg w-full max-w-3xl max-h-[90vh] flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-[#3a3a3a]">
          <h2 className="text-white text-xl tracking-wide">編輯專輯</h2>
          <button
            onClick={onClose}
            className="p-2 text-[#9ca3af] hover:text-white hover:bg-[#3a3a3a] rounded transition-colors"
          >
            <X className="w-5 h-5" strokeWidth={1.5} />
          </button>
        </div>

        {/* Tab Navigation */}
        <div className="flex gap-8 px-6 pt-6 border-b border-[#3a3a3a]">
          <button
            onClick={() => setActiveTab('basic')}
            className={`pb-3 text-sm transition-colors relative ${
              activeTab === 'basic'
                ? 'text-white'
                : 'text-[#9ca3af] hover:text-white'
            }`}
          >
            專輯編輯選項
            {activeTab === 'basic' && (
              <div className="absolute bottom-0 left-0 right-0 h-0.5 bg-[#6366f1]"></div>
            )}
          </button>
          <button
            onClick={() => setActiveTab('preference')}
            className={`pb-3 text-sm transition-colors relative ${
              activeTab === 'preference'
                ? 'text-white'
                : 'text-[#9ca3af] hover:text-white'
            }`}
          >
            詮釋資料偏好設定
            {activeTab === 'preference' && (
              <div className="absolute bottom-0 left-0 right-0 h-0.5 bg-[#6366f1]"></div>
            )}
          </button>
          <button
            onClick={() => setActiveTab('details')}
            className={`pb-3 text-sm transition-colors relative ${
              activeTab === 'details'
                ? 'text-white'
                : 'text-[#9ca3af] hover:text-white'
            }`}
          >
            編輯專輯
            {activeTab === 'details' && (
              <div className="absolute bottom-0 left-0 right-0 h-0.5 bg-[#6366f1]"></div>
            )}
          </button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-6">
          {/* Basic Tab */}
          {activeTab === 'basic' && (
            <div className="space-y-6">
              <div>
                <label className="block text-white text-sm mb-2">標題</label>
                <input
                  type="text"
                  value={formData.title}
                  onChange={(e) => setFormData({ ...formData, title: e.target.value })}
                  className="w-full bg-[#1a1a1a] border border-[#3a3a3a] rounded px-3 py-2 text-white text-sm focus:outline-none focus:border-[#6366f1] focus:ring-1 focus:ring-[#6366f1]/50"
                />
              </div>

              <div>
                <label className="block text-white text-sm mb-2">原始標題</label>
                <input
                  type="text"
                  value={formData.originalTitle || ''}
                  onChange={(e) => setFormData({ ...formData, originalTitle: e.target.value })}
                  className="w-full bg-[#1a1a1a] border border-[#3a3a3a] rounded px-3 py-2 text-white text-sm focus:outline-none focus:border-[#6366f1] focus:ring-1 focus:ring-[#6366f1]/50"
                />
              </div>

              <div>
                <label className="block text-white text-sm mb-2">開發者</label>
                <input
                  type="text"
                  value={formData.developer || ''}
                  onChange={(e) => setFormData({ ...formData, developer: e.target.value })}
                  className="w-full bg-[#1a1a1a] border border-[#3a3a3a] rounded px-3 py-2 text-white text-sm focus:outline-none focus:border-[#6366f1] focus:ring-1 focus:ring-[#6366f1]/50"
                />
              </div>

              <div>
                <label className="block text-white text-sm mb-2">發行日期</label>
                <input
                  type="date"
                  value={formData.releaseDate || ''}
                  onChange={(e) => setFormData({ ...formData, releaseDate: e.target.value })}
                  className="w-full bg-[#1a1a1a] border border-[#3a3a3a] rounded px-3 py-2 text-white text-sm focus:outline-none focus:border-[#6366f1] focus:ring-1 focus:ring-[#6366f1]/50"
                />
              </div>

              <div>
                <label className="block text-white text-sm mb-2">描述</label>
                <textarea
                  value={formData.description || ''}
                  onChange={(e) => setFormData({ ...formData, description: e.target.value })}
                  rows={6}
                  className="w-full bg-[#1a1a1a] border border-[#3a3a3a] rounded px-3 py-2 text-white text-sm focus:outline-none focus:border-[#6366f1] focus:ring-1 focus:ring-[#6366f1]/50 resize-none"
                />
              </div>
            </div>
          )}

          {/* Preference Tab */}
          {activeTab === 'preference' && (
            <div className="space-y-6">
              <div>
                <h3 className="text-white mb-4">元數據來源偏好</h3>
                <div className="space-y-3">
                  <label className="flex items-center gap-3 p-3 bg-[#1a1a1a] border border-[#3a3a3a] rounded cursor-pointer hover:border-[#4a4a4a] transition-colors">
                    <input type="radio" name="source" value="vndb" className="accent-[#6366f1]" defaultChecked />
                    <div>
                      <div className="text-white text-sm">優先使用 VNDB</div>
                      <div className="text-[#9ca3af] text-xs">官方英文標題和詳細資料</div>
                    </div>
                  </label>
                  <label className="flex items-center gap-3 p-3 bg-[#1a1a1a] border border-[#3a3a3a] rounded cursor-pointer hover:border-[#4a4a4a] transition-colors">
                    <input type="radio" name="source" value="bangumi" className="accent-[#6366f1]" />
                    <div>
                      <div className="text-white text-sm">優先使用 Bangumi</div>
                      <div className="text-[#9ca3af] text-xs">中文社群資料</div>
                    </div>
                  </label>
                  <label className="flex items-center gap-3 p-3 bg-[#1a1a1a] border border-[#3a3a3a] rounded cursor-pointer hover:border-[#4a4a4a] transition-colors">
                    <input type="radio" name="source" value="custom" className="accent-[#6366f1]" />
                    <div>
                      <div className="text-white text-sm">自訂</div>
                      <div className="text-[#9ca3af] text-xs">使用我的編輯內容</div>
                    </div>
                  </label>
                </div>
              </div>
            </div>
          )}

          {/* Details Tab */}
          {activeTab === 'details' && (
            <div className="space-y-8">
              {/* Staff Section */}
              <div>
                <h3 className="text-white mb-4">製作團隊</h3>
                <div className="flex items-center gap-3 mb-4 text-[#9ca3af]">
                  <Music className="w-5 h-5" strokeWidth={1.5} />
                  <Plus className="w-4 h-4" strokeWidth={1.5} />
                  <FileText className="w-5 h-5" strokeWidth={1.5} />
                  <span className="text-sm">{formData.staff?.length || 0} 位製作人員</span>
                </div>
                
                {formData.staff && formData.staff.length > 0 && (
                  <div className="space-y-2 mb-4">
                    {formData.staff.map((member, index) => (
                      <div
                        key={index}
                        className="flex items-center justify-between bg-[#1a1a1a] border border-[#3a3a3a] rounded px-4 py-2 group"
                      >
                        <div className="text-sm">
                          <span className="text-white">{member.name}</span>
                          <span className="text-[#9ca3af] mx-2">·</span>
                          <span className="text-[#9ca3af]">{member.role}</span>
                        </div>
                        <button
                          onClick={() => handleRemoveStaff(index)}
                          className="p-1 text-[#9ca3af] hover:text-red-400 opacity-0 group-hover:opacity-100 transition-all"
                        >
                          <Trash2 className="w-4 h-4" strokeWidth={1.5} />
                        </button>
                      </div>
                    ))}
                  </div>
                )}

                <button className="flex items-center gap-2 text-white text-sm hover:text-[#6366f1] transition-colors">
                  <Edit2 className="w-4 h-4" strokeWidth={1.5} />
                  編輯製作人員名單
                </button>
              </div>

              {/* Tags Section */}
              <div>
                <div className="flex items-center justify-between mb-4">
                  <h3 className="text-white">遊戲類型</h3>
                  <label className="flex items-center gap-2 text-sm text-[#9ca3af]">
                    <input type="checkbox" className="accent-[#6366f1]" />
                    以在地語言示音樂類型
                  </label>
                </div>

                {/* Tag Display */}
                <div className="flex items-start gap-2 mb-4">
                  <Music className="w-5 h-5 text-[#9ca3af] mt-1" strokeWidth={1.5} />
                  <div className="flex-1 flex flex-wrap gap-2">
                    {formData.tags?.map((tag, index) => (
                      <button
                        key={index}
                        onClick={() => handleRemoveTag(index)}
                        className="flex items-center gap-2 px-4 py-2 bg-[#6366f1] hover:bg-[#5558e3] text-white rounded-full text-sm transition-colors"
                      >
                        <Plus className="w-3 h-3 rotate-45" strokeWidth={2} />
                        {tag}
                      </button>
                    ))}
                    <button
                      onClick={() => {
                        const tag = prompt('新增標籤：');
                        if (tag) {
                          setFormData({
                            ...formData,
                            tags: [...(formData.tags || []), tag]
                          });
                        }
                      }}
                      className="flex items-center gap-2 px-4 py-2 bg-[#6366f1] hover:bg-[#5558e3] text-white rounded-full text-sm transition-colors"
                    >
                      <Plus className="w-4 h-4" strokeWidth={2} />
                      另類/獨立搖滾
                    </button>
                  </div>
                </div>

                {/* Tag Search Input */}
                <div className="flex items-start gap-2">
                  <FileText className="w-5 h-5 text-[#9ca3af] mt-2" strokeWidth={1.5} />
                  <input
                    type="text"
                    value={newTag}
                    onChange={(e) => setNewTag(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter') {
                        handleAddTag();
                      }
                    }}
                    placeholder="搜尋遊戲類型"
                    className="flex-1 bg-[#1a1a1a] border border-[#3a3a3a] rounded px-4 py-2 text-white text-sm focus:outline-none focus:border-[#6366f1] focus:ring-1 focus:ring-[#6366f1]/50"
                  />
                </div>
              </div>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between px-6 py-4 border-t border-[#3a3a3a]">
          <button
            onClick={handleRevert}
            className="text-[#9ca3af] text-sm hover:text-white transition-colors"
          >
            還原編輯...
          </button>
          <div className="flex gap-3">
            <button
              onClick={onClose}
              className="px-6 py-2 text-white text-sm hover:bg-[#3a3a3a] rounded transition-colors"
            >
              取消
            </button>
            <button
              onClick={handleSave}
              className="px-6 py-2 bg-[#6366f1] hover:bg-[#5558e3] text-white text-sm rounded transition-colors"
            >
              儲存
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}