/**
 * MetadataEditModal - Metadata editing using shadcn/ui Dialog
 * 
 * Design System:
 * - bg-neutral-900 (panel)
 * - border-neutral-800
 * - purple-500 (accent)
 * - rounded-xl
 */

import { useState, useEffect } from 'react';
import { Save, Trash2, X } from 'lucide-react';
import {
      Dialog,
      DialogContent,
      DialogHeader,
      DialogTitle,
      DialogFooter,
} from '../ui/dialog';
import { Button } from '../ui/button';
import { Input } from '../ui/input';
import { Label } from '../ui/label';
import { Badge } from '../ui/badge';
import { WorkshopItem } from './PosterCard';
import { WorkshopStatus } from './TopTabs';
import { MemoData, MemoColor } from './Memo';
import {
      Select,
      SelectContent,
      SelectItem,
      SelectTrigger,
      SelectValue,
} from '../ui/select';

interface MetadataEditModalProps {
      item: WorkshopItem | null;
      isOpen: boolean;
      onClose: () => void;
      onSave: (item: WorkshopItem) => void;
      onDelete?: (id: string) => void;
}

export function MetadataEditModal({
      item,
      isOpen,
      onClose,
      onSave,
      onDelete,
}: MetadataEditModalProps) {
      const [title, setTitle] = useState('');
      const [status, setStatus] = useState<WorkshopStatus>('pending');
      const [tags, setTags] = useState<string[]>([]);
      const [newTag, setNewTag] = useState('');
      const [memos, setMemos] = useState<MemoData[]>([]);
      const [newMemoText, setNewMemoText] = useState('');
      const [newMemoColor, setNewMemoColor] = useState<MemoColor>('red');

      useEffect(() => {
            if (item) {
                  setTitle(item.title || '');
                  setStatus(item.status);
                  setTags(item.tags || []);
                  setMemos(item.memos || []);
            } else {
                  setTitle('');
                  setStatus('pending');
                  setTags([]);
                  setMemos([]);
            }
      }, [item]);

      const handleSave = () => {
            const savedItem: WorkshopItem = {
                  id: item?.id || `new-${Date.now()}`,
                  title: title || undefined,
                  coverImage: item?.coverImage,
                  status,
                  tags: tags.length > 0 ? tags : undefined,
                  memos: memos.length > 0 ? memos : undefined,
            };
            onSave(savedItem);
            onClose();
      };

      const handleAddTag = () => {
            if (newTag.trim() && !tags.includes(newTag.trim())) {
                  setTags([...tags, newTag.trim()]);
                  setNewTag('');
            }
      };

      const handleRemoveTag = (tag: string) => {
            setTags(tags.filter(t => t !== tag));
      };

      const handleAddMemo = () => {
            if (newMemoText.trim()) {
                  const newMemo: MemoData = {
                        id: `memo-${Date.now()}`,
                        text: newMemoText.trim(),
                        color: newMemoColor,
                        isManual: true,
                  };
                  setMemos([...memos, newMemo]);
                  setNewMemoText('');
            }
      };

      const handleRemoveMemo = (id: string) => {
            setMemos(memos.filter(m => m.id !== id));
      };

      return (
            <Dialog open={isOpen} onOpenChange={(open) => !open && onClose()}>
                  <DialogContent className="bg-neutral-900 border-neutral-800 rounded-xl max-w-lg">
                        <DialogHeader>
                              <DialogTitle className="text-white">
                                    {item ? '編輯 Metadata' : '新增作品'}
                              </DialogTitle>
                        </DialogHeader>

                        <div className="space-y-4 max-h-[60vh] overflow-y-auto py-4">
                              {/* Title */}
                              <div className="space-y-2">
                                    <Label className="text-neutral-400">標題</Label>
                                    <Input
                                          value={title}
                                          onChange={(e) => setTitle(e.target.value)}
                                          placeholder="作品標題"
                                          className="bg-neutral-800 border-neutral-700 text-white"
                                    />
                              </div>

                              {/* Status */}
                              <div className="space-y-2">
                                    <Label className="text-neutral-400">狀態</Label>
                                    <div className="flex gap-2">
                                          {(['pending', 'working', 'paused'] as WorkshopStatus[]).map((s) => (
                                                <Button
                                                      key={s}
                                                      variant={status === s ? 'default' : 'outline'}
                                                      size="sm"
                                                      onClick={() => setStatus(s)}
                                                      className={status === s
                                                            ? 'bg-purple-500 text-white hover:bg-purple-600'
                                                            : 'bg-neutral-800 border-neutral-700 text-neutral-400 hover:text-white'
                                                      }
                                                >
                                                      {s === 'pending' ? '未開始' : s === 'working' ? '工作中' : '擱置'}
                                                </Button>
                                          ))}
                                    </div>
                              </div>

                              {/* Tags */}
                              <div className="space-y-2">
                                    <Label className="text-neutral-400">標籤</Label>
                                    <div className="flex flex-wrap gap-2 mb-2">
                                          {tags.map((tag) => (
                                                <Badge
                                                      key={tag}
                                                      variant="secondary"
                                                      className="bg-neutral-800 text-white flex items-center gap-1"
                                                >
                                                      {tag}
                                                      <button onClick={() => handleRemoveTag(tag)} className="hover:text-red-400">
                                                            <X className="w-3 h-3" />
                                                      </button>
                                                </Badge>
                                          ))}
                                    </div>
                                    <div className="flex gap-2">
                                          <Input
                                                value={newTag}
                                                onChange={(e) => setNewTag(e.target.value)}
                                                onKeyDown={(e) => e.key === 'Enter' && handleAddTag()}
                                                placeholder="新增標籤"
                                                className="flex-1 bg-neutral-800 border-neutral-700 text-white"
                                          />
                                          <Button
                                                onClick={handleAddTag}
                                                variant="outline"
                                                className="bg-neutral-800 border-neutral-700 text-white hover:bg-neutral-700"
                                          >
                                                新增
                                          </Button>
                                    </div>
                              </div>

                              {/* Memos */}
                              <div className="space-y-2">
                                    <Label className="text-neutral-400">備忘錄</Label>
                                    <div className="space-y-2 mb-2">
                                          {memos.map((memo) => (
                                                <div
                                                      key={memo.id}
                                                      className={`
                    flex items-center gap-2 px-3 py-2 rounded-lg
                    ${memo.color === 'red' ? 'bg-red-500/15 text-red-400' :
                                                                  memo.color === 'yellow' ? 'bg-yellow-500/15 text-yellow-400' :
                                                                        'bg-neutral-500/15 text-neutral-400'}
                  `}
                                                >
                                                      <span className="text-sm flex-1">– {memo.text}</span>
                                                      {memo.isManual && (
                                                            <button onClick={() => handleRemoveMemo(memo.id)} className="hover:opacity-80">
                                                                  <X className="w-3 h-3" />
                                                            </button>
                                                      )}
                                                </div>
                                          ))}
                                    </div>
                                    <div className="flex gap-2">
                                          <Select value={newMemoColor} onValueChange={(v) => setNewMemoColor(v as MemoColor)}>
                                                <SelectTrigger className="w-24 bg-neutral-800 border-neutral-700 text-white">
                                                      <SelectValue />
                                                </SelectTrigger>
                                                <SelectContent className="bg-neutral-800 border-neutral-700">
                                                      <SelectItem value="red">🔴 紅</SelectItem>
                                                      <SelectItem value="yellow">🟡 黃</SelectItem>
                                                      <SelectItem value="gray">⚪ 灰</SelectItem>
                                                </SelectContent>
                                          </Select>
                                          <Input
                                                value={newMemoText}
                                                onChange={(e) => setNewMemoText(e.target.value)}
                                                onKeyDown={(e) => e.key === 'Enter' && handleAddMemo()}
                                                placeholder="新增備忘錄"
                                                className="flex-1 bg-neutral-800 border-neutral-700 text-white"
                                          />
                                          <Button
                                                onClick={handleAddMemo}
                                                variant="outline"
                                                className="bg-neutral-800 border-neutral-700 text-white hover:bg-neutral-700"
                                          >
                                                新增
                                          </Button>
                                    </div>
                              </div>
                        </div>

                        <DialogFooter className="flex items-center justify-between">
                              {item && onDelete ? (
                                    <Button
                                          variant="ghost"
                                          onClick={() => {
                                                onDelete(item.id);
                                                onClose();
                                          }}
                                          className="text-red-400 hover:text-red-300 hover:bg-red-500/10"
                                    >
                                          <Trash2 className="w-4 h-4 mr-2" />
                                          刪除
                                    </Button>
                              ) : (
                                    <div />
                              )}
                              <div className="flex gap-2">
                                    <Button variant="ghost" onClick={onClose} className="text-neutral-400">
                                          取消
                                    </Button>
                                    <Button onClick={handleSave} className="bg-purple-500 hover:bg-purple-600 text-white">
                                          <Save className="w-4 h-4 mr-2" />
                                          儲存
                                    </Button>
                              </div>
                        </DialogFooter>
                  </DialogContent>
            </Dialog>
      );
}
